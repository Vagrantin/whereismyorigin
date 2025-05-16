use std::env;
use std::process::{Command, Stdio};
use std::io::{self, Read, BufReader};
use regex::Regex;

// Size of chunks to read (8 kB)
const CHUNK_SIZE: usize = 8192;

fn main() -> io::Result<()> {

    // Get command line arguments, excluding the program name
    let args: Vec<String> = env::args().collect();
	
	let videofile = &args[1];
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
			
    println!("Running mxfdump.exe with provided arguments...");
    
    // Create command to execute mxfdump.exe
    let mut cmd = Command::new("./bin/mxfdump.exe");
    
    
    // Configure to capture stdout and stderr
    let mut child = cmd
		.arg(videofile)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    // Extract stdout and stderr handles
    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let stderr = child.stderr.take().expect("Failed to capture stderr");
    
    // Prepare regex pattern
    let pattern = r"\[ k = Origin\s+\r?\n?4b\.02, l =\s+\d+\s+\(\d+\) \]\s+\r?\n?\s+\d+\s+([0-9a-fA-F]{2}(?: [0-9a-fA-F]{2}){7})";
    let regex = match Regex::new(pattern) {
        Ok(re) => re,
        Err(e) => {
            eprintln!("Regex error: {}", e);
            return Ok(());
        }
    };
    
    println!("\n--- PROCESSING STDOUT ---");
    // Process stdout in chunks, piping to standard output
    let mut stdout_matches = Vec::new();
    process_output(BufReader::new(stdout), true, &regex, &mut stdout_matches, &videofile, verbose)?;
	
    if mxferror {
		println!("\n--- PROCESSING STDERR ---");
		// Process stderr in chunks, piping to standard error
		let mut stderr_matches = Vec::new();
		process_output(BufReader::new(stderr), false, &regex, &mut stderr_matches, &videofile, verbose)?;
	}
    
    // Wait for the process to complete
    let status = child.wait()?;
    println!("\nExit status: {}", status);
    
    // Print regex matches
    println!("\n--- REGEX PARSING RESULTS ---");
    if stdout_matches.is_empty() {
        println!("No matches found for the Origin pattern.");
    } else {
        let total_matches = stdout_matches.len();
        println!("Found {} match(es) for Origin pattern:", total_matches);
        
        if !stdout_matches.is_empty() {
            println!("\nMatches from STDOUT:");
            for (idx, m) in stdout_matches.iter().enumerate() {
                println!("Match #{}: {}", idx + 1, m);
            }
        }
    }
    
    Ok(())
}

// Process output stream in chunks, collect regex matches
fn process_output<R: Read>(
    mut reader: BufReader<R>, 
    is_stdout: bool, 
    regex: &Regex, 
    matches: &mut Vec<String>,
	videofile: &String,
	verbose: bool
) -> io::Result<()> {
    // For regex matching across chunk boundaries
    let mut leftover = String::new();
    let mut buffer = [0; CHUNK_SIZE];
    
	print!("Processing {}", &videofile);
    // Read and process in chunks
    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break; // End of stream
        }
        
        // Convert chunk to string, handling UTF-8 errors gracefully
        let chunk = match std::str::from_utf8(&buffer[0..bytes_read]) {
            Ok(s) => s,
            Err(_) => {
                // Handle invalid UTF-8 (just output the bytes we can)
                let string = String::from_utf8_lossy(&buffer[0..bytes_read]).to_string();
                if is_stdout {
                    print!("{}", string);
                } else {
                    eprint!("{}", string);
                }
                continue;
            }
        };
        
        // Print current chunk
        if is_stdout {
			if verbose {
				print!("{}", chunk);
			}
        } else {
            eprint!("{}", chunk);
        }
        
        // Combine leftover with current chunk for regex matching
        leftover.push_str(chunk);
        
        // Extract regex matches from the combined text
        for cap in regex.captures_iter(&leftover) {
            if let Some(hex_value) = cap.get(1) {
                matches.push(hex_value.as_str().to_string());
            }
        }
        
        // Keep the last few bytes for the next iteration
        // This helps with matches that might span chunks
        if leftover.len() > 80 {
            leftover = leftover.chars().skip(leftover.len() - 80).collect();
        }
    }

    Ok(())
}