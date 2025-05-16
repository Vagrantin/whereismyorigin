use regex::Regex;
use std::env;
use std::io::{self, BufReader, Read};
use std::process::{Command, Stdio};
use encoding_rs::UTF_16LE;
use encoding_rs_io::DecodeReaderBytesBuilder;

fn main() -> io::Result<()> {
    // Get the file path from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <media_file_path>", args[0]);
        std::process::exit(1);
    }
	
    let media_file_path = &args[1];
    process_output_from_command("./bin/mxfdump.exe", media_file_path)?;
    
    Ok(())
}

fn process_output_from_command(executable: &str, videofile: &str) -> io::Result<()> {
    // Execute the command and capture its stdout
    let mut output = Command::new(executable)
        .arg(videofile)
        .stdout(Stdio::piped())
		.stderr(Stdio::piped())
        .spawn()?;
		

    // Get the stdout and stderr handle
    let mut stdout = output.stdout.take().expect("Failed to capture stdout"); 
	let mut stderr = output.stderr.take().expect("Failed to capture stderr"); 
	/*
	println!("Mxf dump output\n {:#?}",&stdout);
	println!("Failure\n {:#?}",&stderr);
	*/
    // Read stdout and stderr
    let mut stdout_content = String::new();
    stdout.read_to_string(&mut stdout_content)?;
    
    let mut stderr_content = String::new();
    stderr.read_to_string(&mut stderr_content)?;
    
    // Wait for the process to complete
    let status = output.wait()?;
    
    // Print stdout content
    if !stdout_content.is_empty() {
        println!("\n--- STDOUT ---");
        println!("{}", stdout_content);
    }
    
    // Print stderr content
    if !stderr_content.is_empty() {
        println!("\n--- STDERR ---");
        println!("{}", stderr_content);
	}
	
	// Print exit status
    println!("\nExit status: {}", status);
    
    // The updated regex pattern to match the format and capture the hex bytes
    let pattern = r"\[ k = Origin\s+\r?\n?4b\.02, l =\s+\d+\s+\(\d+\) \]\s+\r?\n?\s+\d+\s+([0-9a-fA-F]{2}(?: [0-9a-fA-F]{2}){7})";
    
    // Create a regex object
    let regex = Regex::new(pattern).unwrap();
    
    // Find all matches in the command output
    let mut found = false;
    for captures in regex.captures_iter(&stdout_content) {
        found = true;
        // Get the captured hex bytes as a string
        if let Some(hex_bytes) = captures.get(1) {
            let hex_bytes = hex_bytes.as_str();
            
            // Check if all bytes are zero
            let all_zeros = hex_bytes
                .split_whitespace()
                .all(|byte| byte == "00");
            
            println!("Found match: '{}'", hex_bytes);
            println!("All bytes are zero: {}", all_zeros);
        }
    }
    
    if !found {
        println!("No matching patterns found in the command output.");
    }
    
    Ok(())
}