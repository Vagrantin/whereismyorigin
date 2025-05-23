use std::env;
use std::process::{Command, Stdio};
use std::io::{self, Read, BufReader};
use std::thread;
use std::sync::mpsc;
use regex::Regex;
use sled::{Config,Db};

mod scan;

// Size of buffer for reading (smaller chunks to avoid memory issues)
const BUFFER_SIZE: usize = 8192;
// Size of the sliding window buffer for regex matching
const WINDOW_SIZE: usize = 1024;

fn main() -> io::Result<()> {
    let mut videofilepath: String = "".to_string();  

    // Get command line arguments, excluding the program name
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <video_folder_path> [-v|--verbose] [-e|--errors]", args[0]);
        return Ok(());
    }
    
    let videofolderpath = &args[1];
    let mut verbose = false;
    let mut mxferror = false;
    
    let mut i = 2;
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
            _ => {
                i+=1;
            }
        }
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
                        println!("Processed {} files so far...", processed_count);
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
    
    // Spawn the process
    let mut child = cmd.spawn()?;
    
    // Extract stdout and stderr handles
    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let stderr = child.stderr.take().expect("Failed to capture stderr");
    
    // Create regex pattern for matching
    let pattern = r"\[ k = Origin\s+\r?\n?4b\.02, l =\s+\d+\s+\(\d+\) \]\s+\r?\n?\s+\d+\s+([0-9a-fA-F]{2}(?: [0-9a-fA-F]{2}){7})";
    let regex = Regex::new(pattern).expect("Invalid regex pattern");
    
    // Create a channel to communicate matches
    let (tx, rx) = mpsc::channel();
    
    // Process stdout in a separate thread - using chunk-based reading for memory efficiency
    thread::spawn(move || {
        let mut reader = BufReader::with_capacity(BUFFER_SIZE, stdout);
        let mut buffer = vec![0; BUFFER_SIZE];
        let mut window = Vec::with_capacity(WINDOW_SIZE);
        
        // Read in chunks to minimize memory usage
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break, // End of file
                Ok(n) => {
                    // Append new data to the sliding window
                    window.extend_from_slice(&buffer[0..n]);
                    
                    // Convert to string for regex matching (for this portion)
                    if let Ok(text) = String::from_utf8(window.clone()) {
                        // Output if verbose
                        if verbose {
                            print!("{}", String::from_utf8_lossy(&buffer[0..n]));
                        }
                        
                        // Check for match
                        if regex.is_match(&text) {
                            tx.send(true).unwrap_or(());
                            return;
                        }
                    }
                    
                    // Trim the window if it's too large, keeping only the tail
                    if window.len() > WINDOW_SIZE {
                        window = window.split_off(window.len() - WINDOW_SIZE);
                    }
                }
                Err(e) => {
                    if verbose {
                        eprintln!("Error reading stdout: {}", e);
                    }
                    break;
                }
            }
        }
        
        // No match found
        tx.send(false).unwrap_or(());
    });
    
    // Always just drain stderr without checking for regex patterns
    thread::spawn(move || {
        let mut reader = BufReader::with_capacity(BUFFER_SIZE, stderr);
        let mut buffer = vec![0; BUFFER_SIZE];
        
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break, // End of file
                Ok(n) => {
                    if verbose && process_errors {
                        eprint!("{}", String::from_utf8_lossy(&buffer[0..n]));
                    }
                }
                Err(e) => {
                    if verbose {
                        eprintln!("Error reading stderr: {}", e);
                    }
                    break;
                }
            }
        }
    });
    
    // Wait for the process to complete
    let status = child.wait()?;
    if verbose {
        println!("Process exit status: {}", status);
    }
    
    // Get the result from the stdout processing thread
    let found_match = rx.recv().unwrap_or(false);
    
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
