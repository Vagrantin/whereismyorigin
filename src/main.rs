use std::env;
use std::process::{self,Command, Stdio};
use std::io::{self, Read, BufReader};
use std::thread;
use std::sync::mpsc;
use regex::Regex;
use sled::Config;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
/*
#[cfg(windows)]
use std::os::windows::process::ExitStatusExt;
*/

mod scan;
mod report;

// Size of buffer for reading (smaller chunks to avoid memory issues)
const BUFFER_SIZE: usize = 8192;
// Size of the sliding window buffer for regex matching
const WINDOW_SIZE: usize = 512;

fn main() -> io::Result<()> {
    let mut videofilepath: String;  

    // Get command line arguments, excluding the program name
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <video_folder_path> [-v|--verbose] [-e|--errors]", args[0]);
        eprintln!("       {} --report (to generate report only)", args[0]);
        eprintln!("Author: Matthieu Ducorps");
        return Ok(());
    }
    
    let videofolderpath = &args[1];
    let mut verbose = false;
    let mut mxferror = false;
    let mut report_mode = false;
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-v" | "--verbose" => {
                verbose = true;
                i+=1;
            }
            "-e" | "--errors" => {
                mxferror = true;
                i+=1;
            }
            "--report" => {
                report_mode = true;
                i+=1;
            }
            _ => {
                i+=1;
            }
        }
    }

    //Check for conflicting parameters
    if report_mode && (verbose || mxferror) {
        eprintln!("Report mode cannot be use with other parameters");
        eprintln!("Usage for report: {} --report", args[0]);
        process::exit(0);
    }

    if report_mode {
        println!("Generating report of files with Origin/Precharge");
        let _ = report::generate_report();
        process::exit(0);
    }
            
    println!("Running the folder scan for MXF files...");
    scan::scandir(videofolderpath, verbose);

    // Initialize the sled database
    let db = Config::new()
        .path("./file_paths_db")
        .open()
        .unwrap();

    println!("\nIterating over all entries in DB...");
    println!("Running mxfdump.exe with provided arguments...");
    
    let mut processed_count = 0;
    let mut found_matches_count = 0;
    
    for result in db.iter() {
        match result {
            Ok((key_bytes, _value_bytes)) => {
                if let Ok(videofilepathb64) = String::from_utf8(key_bytes.to_vec()) {
                    let decodedvideofilepath = decodeb64(&videofilepathb64);
                    match decodedvideofilepath {
                        Ok(val) => {
                            videofilepath = val;
                        }
                        Err(e) => {
                            eprintln!("Error decoding {} : {}", &videofilepathb64, e);
                            continue; // Skip this file
                        }
                    }
                    
                    processed_count += 1;
                    if verbose {
                        println!("This is the path I got: {}", &videofilepath);
                    } else if processed_count % 5 == 0 {
                        println!("Processed {} files so far...", processed_count - 1);
                    }
                    
                    println!("Processing {}", &videofilepath);
                    
                    // Run the external process with the necessary parameters
                    let has_match = process_file_with_mxfdump(&videofilepath, verbose, mxferror)?;
                    
                    if has_match {
                        found_matches_count += 1;
                        // Set the value in the database
                        let originpresent = true;
                        let videofilepathb64 = base64::encode(videofilepath.as_bytes());
                        let _ = db.insert(&videofilepathb64.as_bytes(), originpresent.to_string().as_bytes());
                        
                        println!("Found Origin/Precharge pattern in: {}", &videofilepath);
                    } else if verbose {
                        println!("No Origin/Precharge pattern found in: {}", &videofilepath);
                    }
                } else {
                    eprintln!("Couldn't decode the key");
                }
            }
            Err(e) => {
                eprintln!("Error during iteration {e}");
            }
        }
    }
    
    println!("Processing complete. Processed {} files total.", processed_count);
    println!("Found Origin/Precharge pattern in {} files.", found_matches_count);
    Ok(())
}


fn process_file_with_mxfdump(file_path: &str, verbose: bool, process_errors: bool) -> io::Result<bool> {
    // Create command to execute mxfdump.exe
    let mut cmd = Command::new("./bin/mxfdump.exe");
    cmd.arg(file_path);
    
    // Configure to capture stdout and stderr
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    
    // On Windows, set creation flags to allow clean process termination
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x00000200); // CREATE_NEW_PROCESS_GROUP
    }
    
    // Spawn the process
    let mut child = cmd.spawn()?;
    
    // Extract stdout and stderr handles
    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let stderr = child.stderr.take().expect("Failed to capture stderr");
    
    // Create regex patterns
    let pattern = r"\[ k = Origin\s+\r?\n?4b\.02, l =\s+\d+\s+\(\d+\) \]\s+\r?\n?\s+\d+\s+([0-9a-fA-F]{2}(?: [0-9a-fA-F]{2}){7})";
    let regex = Regex::new(pattern).expect("Invalid regex pattern");
    
    let mxf_opatom_pattern = r"Operational\s+Pattern\s+=\s+06.0e.2b.34.04.01.01.02.0d.01.02.01.10.02.00.00";
    let audio_opatom_regex = Regex::new(mxf_opatom_pattern).expect("Invalid regex pattern");
    let mxf_one_container = r"EssenceContainers\s+=\s+\[\s+count\s+=\s+1\s+\]";
    let audio_container_regex = Regex::new(mxf_one_container).expect("Invalid regex pattern");
    
    // Create channel for results and atomic bool for kill signal
    let (result_tx, result_rx) = mpsc::channel();
    let should_kill = Arc::new(AtomicBool::new(false));
    
    // Process stdout in a separate thread
    let stdout_result_tx = result_tx.clone();
    let stdout_should_kill = should_kill.clone();
    thread::spawn(move || {
        let mut reader = BufReader::with_capacity(BUFFER_SIZE, stdout);
        let mut buffer = vec![0; BUFFER_SIZE];
        let mut window = Vec::with_capacity(WINDOW_SIZE);
        let mut counter: u32 = 0;

        'outer: loop {
            // Check if we should terminate
            if stdout_should_kill.load(Ordering::Relaxed) {
                break 'outer;
            }
            
            match reader.read(&mut buffer) {
                Ok(0) => break 'outer, // End of file
                Ok(n) => {
                    // Append new data to the sliding window
                    window.extend_from_slice(&buffer[0..n]);
                    
                    // Convert to string for regex matching
                    if let Ok(text) = String::from_utf8(window.clone()) {
                        if counter == 0 {
                            // Check if it is an audio file
                            if !is_video_file(&text, &audio_container_regex, &audio_opatom_regex) {
                                println!("Audio file detected, stopping MXFDump");
                                // Signal to kill the process
                                stdout_should_kill.store(true, Ordering::Relaxed);
                                // Send result indicating we should skip this file
                                let _ = stdout_result_tx.send(false);
                                break 'outer;
                            }
                        }
                        
                        // Check for match
                        if is_valid_mxf_dump_chunk(&text, &regex) {
                            println!("Found Origin/Precharge pattern");
                            let _ = stdout_result_tx.send(true);
                            break 'outer;
                        }

                        // Output if verbose
                        if verbose {
                            print!("{}", String::from_utf8_lossy(&buffer[0..n]));
                        }
                    }
                    
                    // Trim the window if it's too large
                    if window.len() > WINDOW_SIZE {
                        window = window.split_off(window.len() - WINDOW_SIZE);
                    }
                    counter += 1;
                }
                Err(e) => {
                    if verbose {
                        eprintln!("Error reading stdout: {}", e);
                    }
                    break 'outer;
                }
            }
        }
        
        // If we get here without sending a result, send false
        let _ = stdout_result_tx.send(false);
    });

    // Process stderr in a separate thread
    let stderr_should_kill = should_kill.clone();
    thread::spawn(move || {
        let mut reader = BufReader::with_capacity(BUFFER_SIZE, stderr);
        let mut buffer = vec![0; BUFFER_SIZE];
        
        loop {
            // Check if we should terminate
            if stderr_should_kill.load(Ordering::Relaxed) {
                break;
            }
            
            match reader.read(&mut buffer) {
                Ok(0) => break, // End of file
                Ok(n) => {
                    if verbose && process_errors {
                        eprint!("{}", String::from_utf8_lossy(&buffer[0..n]));
                    }
                }
                Err(_) => break,
            }
        }
    });
    
    // Wait for either a result or a timeout
    let found_match = match result_rx.recv_timeout(std::time::Duration::from_secs(30)) {
        Ok(result) => {
            // If we got a kill signal, terminate the process
            if should_kill.load(Ordering::Relaxed) {
                let _ = child.kill();
            }
            result
        }
        Err(_) => {
            // Timeout - kill the process and return false
            println!("Process timed out after 30 seconds, killing MXFDump");
            should_kill.store(true, Ordering::Relaxed);
            let _ = child.kill();
            false
        }
    };
    
    // Wait for the process to complete (with timeout)
    match child.try_wait() {
        Ok(Some(status)) => {
            if verbose {
                println!("Process exit status: {}", status);
            }
        }
        Ok(None) => {
            // Process is still running, kill it forcefully on Windows
            if verbose {
                println!("Force killing process...");
            }
            let _ = child.kill();
            // Give it a moment to die gracefully
            std::thread::sleep(std::time::Duration::from_millis(100));
            let _ = child.wait();
        }
        Err(e) => {
            if verbose {
                eprintln!("Error checking process status: {}", e);
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    
    Ok(found_match)
}

fn decodeb64(file_path_b64: &String) -> Result<String,String> {
    match base64::decode(&file_path_b64) {
        Ok(decoded_bytes) => {
            match String::from_utf8(decoded_bytes){
                Ok(decoded_string) => Ok(decoded_string),
                Err(err) => {
                    eprintln!("Couldn't decode the base64 value as UTF-8 {err}");
                    Err("Invalid UTF-8".to_string())
                }
            }
        }
        Err(err) => {
            eprintln!("Couldn't decode the base64 value {err}");
            Err("Invalid UTF-8".to_string())
        }
    }
}

fn is_valid_mxf_dump_chunk(mxf_dump_chunk: &str, regex: &Regex) -> bool {

    if let Some(captures) = regex.captures(mxf_dump_chunk) {
        // Get the captured hexadecimal string (group 1)
        if let Some(hex_match) = captures.get(1) {
            let hex_string = hex_match.as_str();
            // Check if the captured string is exactly the "all zeros" pattern
            if hex_string == "00 00 00 00 00 00 00 00" {
                return false; // Matched, but hex is all zeros, so invalid
            } else {
                return true; // Matched and hex is not all zeros, so valid
            }
        }
    }
    false // No match found at all
}

fn is_video_file(mxf_dump_chunk: &str, audio_container_regex: &Regex, audio_opatom_regex: &Regex) -> bool {

    if let Some(_captures) = audio_opatom_regex.captures(mxf_dump_chunk) {
        if let Some(_captures) = audio_container_regex.captures(mxf_dump_chunk) {
            println!("this is an audio file we skip it.");
            return false;
        } else {
            return true;
        } 
    } else {
        return true;
    }
}

