use regex::Regex;
use std::env;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use encoding_rs::UTF_16LE;
use encoding_rs_io::DecodeReaderBytesBuilder;

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
    
    // First, read a few bytes to check for BOM
    let mut first_bytes = [0; 2];
    {
        let mut file_clone = file.try_clone()?;
        file_clone.read_exact(&mut first_bytes)?;
    }
    
    // Check if file starts with UTF-16 LE BOM (FF FE)
    let _has_bom = first_bytes[0] == 0xFF && first_bytes[1] == 0xFE;
    
    // Now read and decode the file content
    let mut content = String::new();
    let mut decoder = DecodeReaderBytesBuilder::new()
        .encoding(Some(UTF_16LE))
        .build(file);
    
    decoder.read_to_string(&mut content)?;
    
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