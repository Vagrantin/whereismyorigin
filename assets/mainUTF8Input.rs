use regex::Regex;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

fn main() -> io::Result<()> {
    // Get the file path from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <file_path>", args[0]);
        std::process::exit(1);
    }
    
    let file_path = &args[1];
    process_file(file_path)?;
    
    Ok(())
}

fn process_file(file_path: &str) -> io::Result<()> {
    let path = Path::new(file_path);
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    
    // Read the entire file content into a string
    let mut content = String::new();
    for line in reader.lines() {
        content.push_str(&line?);
        content.push('\n');
    }
    
    // The regex pattern to match the format and capture the hex bytes
    let pattern = r"\[ k = Origin\s+\r?\n?4b\.02, l =\s+\d+\s+\(\d+\) \]\s+\r?\n?\s+\d+\s+([0-9a-fA-F]{2}(?: [0-9a-fA-F]{2}){7})";
    
    // Create a regex object
    let regex = Regex::new(pattern).unwrap();
    
    // Find all matches in the file content
    let mut found = false;
    for captures in regex.captures_iter(&content) {
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
        println!("No matching patterns found in the file.");
    }
    
    Ok(())
}