use std::io;
use sled::Config;
use base64;

pub fn generate_report() -> io::Result<()> {
    let equal_divider = "=";
    let minus_divider = "-";
    // Initialize the sled database
    let db = match Config::new()
        .path("./file_paths_db")
        .open() {
        Ok(database) => database,
        Err(e) => {
            eprintln!("Error: Could not open database. Make sure you have run the analyzer at least once.");
            eprintln!("Database error: {}", e);
            return Ok(());
        }
    };

    println!("{}", equal_divider.repeat(80));
    println!("MXF ANALYZER REPORT - Files with Origin/Precharge Patterns");
    println!("{}", equal_divider.repeat(80));
    println!();

    let mut files_with_patterns = Vec::new();
    let mut total_files = 0;
    let mut files_with_patterns_count = 0;

    // Iterate through all entries in the database
    for result in db.iter() {
        match result {
            Ok((key_bytes, value_bytes)) => {
                total_files += 1;
                
                // Decode the file path from base64
                if let Ok(file_path_b64) = String::from_utf8(key_bytes.to_vec()) {
                    let decoded_file_path = match decode_b64(&file_path_b64) {
                        Ok(path) => path,
                        Err(e) => {
                            eprintln!("Warning: Could not decode file path {}: {}", file_path_b64, e);
                            continue;
                        }
                    };

                    // Check the value to see if patterns were found
                    if let Ok(value_str) = String::from_utf8(value_bytes.to_vec()) {
                        // Check if the value indicates a pattern was found
                        if value_str.trim() == "true" {
                            files_with_patterns.push(decoded_file_path);
                            files_with_patterns_count += 1;
                        }
                    }
                } else {
                    eprintln!("Warning: Could not decode database key as UTF-8");
                }
            }
            Err(e) => {
                eprintln!("Warning: Error reading database entry: {}", e);
            }
        }
    }

    // Display the results
    if files_with_patterns_count == 0 {
        println!("No files with Origin/Precharge patterns found in the database.");
        if total_files > 0 {
            println!("Total files in database: {}", total_files);
            println!();
            println!("This could mean:");
            println!("- No MXF files have been analyzed yet");
            println!("- No files contain the Origin/Precharge pattern");
            println!("- Analysis is still in progress");
        } else {
            println!("The database appears to be empty.");
            println!("Run the analyzer first: mxf_analyzer <folder_path>");
        }
    } else {
        println!("Found {} file(s) with Origin/Precharge patterns:", files_with_patterns_count);
        println!();

        // Sort the file paths for better readability
        files_with_patterns.sort();

        for (index, file_path) in files_with_patterns.iter().enumerate() {
            println!("{}. {}", index + 1, file_path);
        }

        println!();
        println!("{}", minus_divider.repeat(80));
        println!("SUMMARY:");
        println!("Total files in database: {}", total_files);
        println!("Files with Origin/Precharge patterns: {}", files_with_patterns_count);
        println!("Files without patterns: {}", total_files - files_with_patterns_count);
        
        if files_with_patterns_count > 0 {
            let percentage = (files_with_patterns_count as f64 / total_files as f64) * 100.0;
            println!("Percentage with patterns: {:.1}%", percentage);
        }
    }

    println!();
    println!("{}", equal_divider.repeat(80));
    println!("Report generated successfully.");
    println!("{}", equal_divider.repeat(80));

    Ok(())
}

fn decode_b64(file_path_b64: &str) -> Result<String, String> {
    match base64::decode(file_path_b64) {
        Ok(decoded_bytes) => {
            match String::from_utf8(decoded_bytes) {
                Ok(decoded_string) => Ok(decoded_string),
                Err(err) => {
                    Err(format!("Could not decode base64 value as UTF-8: {}", err))
                }
            }
        }
        Err(err) => {
            Err(format!("Could not decode base64 value: {}", err))
        }
    }
}
