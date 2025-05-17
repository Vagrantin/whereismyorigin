//scan directory
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use walkdir::WalkDir;
use sled::Config;
use base64;


pub fn scandir(directory: &String, verbose: bool) {
    let dir_path = Path::new(&directory);
    let (tx, rx) = mpsc::channel();

    // Spawn a thread to scan the directory
    let dir_path_clone = dir_path.to_path_buf();
    thread::spawn(move || scan_directory(&dir_path_clone, tx));

    // Initialize the sled database
    let db = Config::new()
        .path("./file_paths_db")
        .open()
        .unwrap();

    // Receive and process file paths
    for file_path in rx {
        let file_path_str = file_path.to_string_lossy().into_owned();
        let pattern_found = 0u8; //By default we assume their is no Origin/Precharge
        
        let file_path_b64 = base64::encode(file_path_str.as_bytes());
        if verbose {
            println!("This is the b64 version {file_path_b64}");

            match base64::decode(&file_path_b64) {
                Ok(decoded_bytes) => {
                    match String::from_utf8(decoded_bytes) {
                        Ok(decoded_string) => {
                            println!("This the decoded b64 version {decoded_string}");
                        }
                        Err(_) => {
                            eprintln!("kawabunga");
                        }
                    }
                }
                Err(_) => {
                    eprintln!("Moto kawabunga");
                }
            }
        }

        // Check if the file already exists in the database
        if let Ok(Some(_value)) = db.get(file_path_b64.as_bytes()) {
                if verbose {
                    println!("File already exists in database: {}", file_path.display());
                }
            } else {
                if verbose {
                    println!("Found new MXF file: {}", file_path.display());
                }
                // Save the file path to the sled database if doesn't already exist
                // When a file path is added it's value is systematically set to false
                // we assume there is no Origin/Precharge by default
                let _ = db.insert(file_path_b64.as_bytes(), &[pattern_found]);
            }
    }
    
    // Flush all changes to disk before exiting
    db.flush().unwrap();
}

fn scan_directory(dir: &Path, tx: mpsc::Sender<PathBuf>) {
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path = entry.path().to_path_buf();
            if path.extension().and_then(|s| s.to_str()) == Some("mxf") {
                tx.send(path).unwrap();
            }
        }
    }
}

