# MXF File Analyzer

A Rust command-line tool for scanning directories containing MXF (Material eXchange Format) video files and analyzing them to identify if there is Origin/Precharge patterns using the mxfdump utility.

## Overview

This tool performs a two-phase analysis of MXF files:

1. **Directory Scan**: Recursively scans a specified directory for `.mxf` files and stores their paths in a local database
2. **Pattern Analysis**: Processes each MXF file using `mxfdump.exe` to detect specific Origin/Precharge patterns in the metadata

The tool is designed to handle large collections of MXF files hopefully efficiently and audio file filtering.

## Features

- **Recursive Directory Scanning**: Finds all MXF files in subdirectories
- **Persistent Database**: Uses a Sled database to track processed files and avoid re-scanning
- **Pattern Detection**: Searches for specific Origin/Precharge metadata patterns
- **Audio File Filtering**: Automatically skips audio-only MXF files to focus on video content
- **Process Management**: Handles external `mxfdump.exe` processes with timeouts and proper cleanup
- **Progress Tracking**: Shows processing progress and final statistics
- **Verbose Output**: Optional detailed logging for debugging and monitoring

## Prerequisites

- Rust toolchain (for building from source)
- `mxfdump.exe` utility placed in `./bin/` directory relative to the executable
- Windows environment (due to mxfdump.exe dependency)

## Dependencies

The tool relies on several Rust crates:

- `sled` - Embedded database for file path storage
- `regex` - Pattern matching for metadata analysis  
- `walkdir` - Recursive directory traversal
- `base64` - File path encoding for database keys

## Usage

```Powershell
whereismyorigin <video_folder_path> [OPTIONS]
```

### Arguments

- `<video_folder_path>` - Path to the directory containing MXF files to analyze

### Options

- `-v, --verbose` - Enable verbose output showing detailed processing information
- `-e, --errors` - Enable error output from mxfdump processes

### Examples

```bash
# Basic usage
whereismyorigin C:\Videos\MXF_Collection

# With verbose output
whereismyorigin C:\Videos\MXF_Collection --verbose

# With verbose output and error reporting
whereismyorigin C:\Videos\MXF_Collection --verbose --errors
```

## How It Works

### Phase 1: Directory Scanning

1. Recursively walks through the specified directory
2. Identifies all files with `.mxf` extension
3. Encodes file paths using Base64 and stores them in a Sled database
4. Skips files that have already been processed in previous runs

### Phase 2: Pattern Analysis

For each MXF file found:

1. Launches `mxfdump.exe` as an external process
2. Reads the output in real-time using buffered streams
3. First checks if the file is audio-only (skips if true)
4. Searches for Origin/Precharge pattern using regex
5. Updates the database with results
6. Handles process timeouts (30 seconds) and cleanup

### Pattern Detection

The tool searches for this specific pattern in mxfdump output:

```regex
\[ k = Origin\s+\r?\n?4b\.02, l =\s+\d+\s+\(\d+\) \]\s+\r?\n?\s+\d+\s+([0-9a-fA-F]{2}(?: [0-9a-fA-F]{2}){7})
```

This pattern identifies Origin metadata with non-zero hexadecimal values  
meaning there is frames in the "Origin" metadata, filtering out empty/zero patterns.

### Audio File Detection

Audio-only MXF files are identified by these patterns:
- Identify if the MXF is an OpAtom: `Operational Pattern: 06.0e.2b.34.04.01.01.02.0d.01.02.01.10.02.00.00`
- Check if the MXF has only one essence container

## Output

The tool provides:

- Count of processed files
- Count of files with Origin/Precharge patterns found
- File paths where patterns were detected
- Optional verbose output showing detailed processing steps

## Performance Considerations

- Uses streaming buffer processing to handle large mxfdump outputs
- Implements sliding window technique for efficient regex matching
- Process timeout prevents hanging on problematic files
- Database prevents re-processing files in subsequent runs
- Multi-threaded design for concurrent stdout/stderr processing

## Database

The tool creates a `file_paths_db` directory containing a Sled database that stores:
- **Keys**: Base64-encoded file paths
- **Values**: Boolean flags indicating whether Origin/Precharge patterns were found

This database persists between runs, allowing incremental processing of large file collections.

## Building from Source

```bash
# Clone the repository
git clone <repository-url>
cd mxf-analyzer

# Build the project
cargo build --release

# Run the executable
./target/release/whereismyorigin <video_folder_path>
```

## Troubleshooting

### Common Issues

1. **"mxfdump.exe not found"**: Ensure `mxfdump.exe` is placed in `./bin/` directory
2. **Permission errors**: Run with appropriate file system permissions
3. **Process timeouts**: Some large MXF files may require longer processing times
4. **Database errors**: Delete `file_paths_db` directory to reset the database

### Verbose Mode

Use `--verbose` flag to get detailed information about:
- File paths being processed
- mxfdump output streams
- Pattern matching results
- Process exit statuses

## License

GPLv3
