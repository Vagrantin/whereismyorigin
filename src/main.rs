use std::env;
use std::process::{Command, Stdio};
use std::io::{self, Read, BufReader};
use regex::Regex;
use sled::{Config,Db};

mod scan;

// Size of chunks to read (8 kB)
const CHUNK_SIZE: usize = 8192;

fn main() -> io::Result<()> {
    let mut videofilepath: String = "".to_string();  

    // Get command line arguments, excluding the program name
    let args: Vec<String> = env::args().collect();
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
			
    println!("Running the folder scan, for MXF files...");
    scan::scandir(videofolderpath, verbose);

    // Initialize the sled database
    let db = Config::new()
        .path("./file_paths_db")
        .open()
        .unwrap();

    // Create command to execute mxfdump.exe
    let mut cmd = Command::new("./bin/mxfdump.exe");

    println!("\nIterating over all entries in DB...");
    println!("Running mxfdump.exe with provided arguments...");
    for result in db.iter() {
        match result {
            Ok((key_bytes, _value_bytes)) => {
                if let Ok(videofilepathb64) = String::from_utf8(key_bytes.to_vec()) {
                    let decodedvideofilepath = decodeb64(&videofilepathb64);
                    match decodedvideofilepath {
                        Ok(val) => {
                            videofilepath = val
                        }
                        Err(e) => eprintln!("Error decoding {} : {}", &videofilepathb64, e)
                    }
                    if verbose {
                        println!("This is the path I got: {}",&videofilepath);
                    }
                    // Configure to capture stdout and stderr
                    let mut child = cmd
            	    	.arg(&videofilepath)
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
                    
                    //println!("\n--- PROCESSING MXFDump output ---");
                    // Process stdout in chunks, piping to standard output
                    let mut stdout_matches = Vec::new();
                    process_output(BufReader::new(stdout), true, &regex, &mut stdout_matches, &videofilepath, verbose, &db)?;
            	    
                    if mxferror {
            	    	println!("\n--- PROCESSING MXFDump Error output ---");
            	    	// Process stderr in chunks, piping to standard error
            	    	let mut stderr_matches = Vec::new();
            	    	process_output(BufReader::new(stderr), false, &regex, &mut stderr_matches, &videofilepath, verbose, &db)?;
            	    }
                    
                    // Wait for the process to complete
                    let status = child.wait()?;
                    if verbose {
                        println!("\nExit status: {}", status);
                    }
                    
                    // Print regex matches
                    println!("\n--- Looking for Origin/Precharge ---");
                    if stdout_matches.is_empty() {
                        println!("No matches found for the Origin,Precharge pattern.\n");
                    } else {
                        let total_matches = stdout_matches.len();
                        println!("Found {} match(es) for Origin, Precharge pattern.\n", total_matches);
                        
                        if verbose {
                            if !stdout_matches.is_empty() {
                                println!("\nMatches from MXFDump output:");
                                for (idx, m) in stdout_matches.iter().enumerate() {
                                    println!("Match #{}: {}", idx + 1, m);
                                }
                            }
                        }
                    }
                    //println!("my key {videofilepath}")
                } else {
                    eprintln!("couldn't decode the key");
                }
            }
            Err(e) => {
                eprintln!("Error during iteration {e}");
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
	videofilepath: &String,
	verbose: bool,
    db: &Db
) -> io::Result<()> {
    // For regex matching across chunk boundaries
    let mut leftover = String::new();
    let mut buffer = [0; CHUNK_SIZE];
    
	print!("Processing {}", &videofilepath);
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
                //maybe we want to stop at first match by default
                matches.push(hex_value.as_str().to_string());
                //Set the value of the file path to true ( we have found Origin/Precharge in the MXF )
                let originpresent = true;
                let videofilepathb64 = base64::encode(videofilepath.as_bytes());
                let _ = db.insert(&videofilepathb64.as_bytes(), originpresent.to_string().as_bytes());
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

fn decodeb64(file_path_b64: &String) -> Result<String,String> {
    match base64::decode(&file_path_b64) {
        Ok(decoded_bytes) => {
            match String::from_utf8(decoded_bytes){
                Ok(decoded_string) => Ok(decoded_string),
                Err(err) => {
                    eprintln!("Couldn't decode the base64 value as UTF-8 {err}");
                    Err("Invalid UTF-8".to_string())
                }
            }
        }
        Err(err) => {
            eprintln!("Couldn't decode the base64 value {err}");
            Err("Invalid UTF-8".to_string())
        }
    }
}
