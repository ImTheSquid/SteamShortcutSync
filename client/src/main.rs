//! Steam Shortcut Sync Client
//! 
//! A simple executable for signaling the Steam Shortcut Sync daemon through a Unix socket.
//! 
//! ## Exit Codes
//! `1`: The `XDG_RUNTIME_DIR` environment variable is not defined.
//! 
//! `2`: The Steam Shortcut Sync daemon is not running.
//! 
//! `3`: There was an error opening the Unix socket stream.
//! 
//! `4`: There was an error signaling the daemon.

use std::{env, process, os::unix::net::UnixStream, path::Path, io::Write};

fn main() {
    println!("Steam Shortcut Sync Client v.{}", option_env!("CARGO_PKG_VERSION").unwrap_or("UNKNOWN"));

    // Open a socket to communicate with daemon
    let key = match env::var("XDG_RUNTIME_DIR") {
        Ok(val) => val,
        Err(e) => {
            eprintln!("Environment variable error for XDG_RUNTIME_DIR: {}", e);
            process::exit(1);
        }
    };

    let path = Path::new(&key).join("steam-shortcut-sync.sock");

    if !path.exists() {
        eprintln!("Steam Shortcut Sync daemon is not running! Please start it first.");
        process::exit(2);
    }

    let mut stream = match UnixStream::connect(&path) {
        Ok(sock) => sock,
        Err(e) => {
            eprintln!("Error opening stream: {}", e);
            process::exit(3);
        }
    };

    match stream.write_all(b"RUN_SYNC") {
        Err(e) => {
            eprintln!("Failed to signal daemon: {}", e);
            process::exit(4);
        }
        Ok(()) => println!("Synchronization requested")
    }
}