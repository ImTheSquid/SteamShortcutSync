use std::{env, process, os::unix::net::UnixStream, path::Path, io::Write};

fn main() {
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
        Ok(()) => println!("Synchronization started")
    }
}