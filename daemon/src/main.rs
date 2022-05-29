use std::{sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc}, process, time::Duration};

use steam_shortcut_sync::{Synchronizer, FileChangeListener, SocketListener};

fn main() {
    println!("Steam Shortcut Sync v.{}", option_env!("CARGO_PKG_VERSION").unwrap_or("UNKNOWN"));

    let run = Arc::new(AtomicBool::new(true));
    let r= Arc::clone(&run);

    // Ctrl+C handling
    match ctrlc::set_handler(move || {
        println!("Interrupt!");
        r.store(false, Ordering::SeqCst);
    }) {
        Ok(_) => {},
        Err(e) => {
            eprintln!("Failed to set interrupt handler! {}", e);
            process::exit(1);
        }
    }

    let (sender, receiver) = mpsc::channel();

    let mut sync = match Synchronizer::new(receiver, Arc::clone(&run), Duration::from_secs(5)) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("Error creating synchronizer!");
            process::exit(2);
        }
    };

    let mut file_watcher = match FileChangeListener::new(sender.clone(), Arc::clone(&run), Duration::from_secs(5)) {
        Ok(f) => f,
        Err(_) => {
            eprintln!("Error creating file watcher!");
            process::exit(3);
        }
    };

    let mut socket_watcher = match SocketListener::new(sender.clone(), Arc::clone(&run), Duration::from_secs(5)) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("Error creating socket watcher!");
            process::exit(4);
        }
    };

    println!("Startup Complete");

    // Initial run on startup
    sender.send(()).unwrap();

    file_watcher.join();
    socket_watcher.join();
    sync.join();

    println!("Shutdown Complete");
}