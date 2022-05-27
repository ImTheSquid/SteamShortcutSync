use std::{sync::{atomic::{AtomicBool, Ordering}, mpsc, Arc, Mutex}, thread, os::unix::net::UnixListener, env, path::Path, io::{self, Read}, process};

pub struct Synchronizer {

}

pub struct SynchronizerCreationError {
    pub kind: SynchronizerCreationErrorKind
}

#[derive(Debug, Clone)]
pub enum SynchronizerCreationErrorKind {
    
}

impl Synchronizer {
    pub fn new(receiver: mpsc::Receiver<()>) -> Result<Synchronizer, SynchronizerCreationError> {


        Ok(Synchronizer {})
    }
}

pub struct FileChangeListener {
    thread: Option<thread::JoinHandle<()>>
}

pub struct FileChangeListenerCreationError {
    pub kind: FileChangeListenerCreationErrorKind
}

#[derive(Debug, Clone)]
pub enum FileChangeListenerCreationErrorKind {
    NoHomeDir,
    NoSteamDir,
    NoApplicationsDir
}

impl FileChangeListener {
    pub fn new(sender: mpsc::Sender<()>, run: Arc<Mutex<AtomicBool>>) -> Result<FileChangeListener, FileChangeListenerCreationError> {
        // Currently only looks at Flatpak directory
        let key = match env::var("HOME") {
            Ok(val) => val,
            Err(_) => return Err(FileChangeListenerCreationError { kind: FileChangeListenerCreationErrorKind::NoHomeDir })
        };

        let steam_path = Path::new(&key).join(".var/app/com.valvesoftware.Steam/data/icons");

        if !steam_path.is_dir() {
            return Err(FileChangeListenerCreationError { kind: FileChangeListenerCreationErrorKind::NoSteamDir });
        }

        let desktop_path = Path::new(&key).join(".local/share/applications");

        if !desktop_path.is_dir() {
            return Err(FileChangeListenerCreationError { kind: FileChangeListenerCreationErrorKind::NoApplicationsDir });
        }

        let thread = thread::spawn(|| {
            
        });

        Ok(FileChangeListener { thread: Some(thread) })
    }

    pub fn join(&mut self) {
        if let Some(thread) = self.thread.take() {
            thread.join().expect("Unable to join thread");
        }
    }
}

pub struct SocketListener {
    thread: Option<thread::JoinHandle<()>>
}

#[derive(Debug, Clone)]
pub struct SocketListenerCreationError {
    pub kind: SocketListenerCreationErrorKind
}

#[derive(Debug, Clone)]
pub enum SocketListenerCreationErrorKind {
    NoRuntimeDir,
    NoSocket
}

impl SocketListener {
    pub fn new(sender: mpsc::Sender<()>, run: Arc<Mutex<AtomicBool>>) -> Result<SocketListener, SocketListenerCreationError> {
        // Attempt to load env var
        let key = match env::var("XDG_RUNTIME_DIR") {
            Ok(val) => val,
            Err(_) => return Err(SocketListenerCreationError { kind: SocketListenerCreationErrorKind::NoRuntimeDir })
        };
    
        let path = Path::new(&key).join("steam-shortcut-sync.sock");

        // Create a socket listener on a separate thread
        let listener = match UnixListener::bind(path) {
            Ok(sock) => sock,
            Err(_) => return Err(SocketListenerCreationError { kind: SocketListenerCreationErrorKind::NoSocket })
        };

        match listener.set_nonblocking(true) {
            Err(e) => {
                eprintln!("Failed to set listener to non-blocking: {}", e);
                process::exit(5);
            }
            Ok(()) => {}
        }

        // Create thread and wait for socket commands
        let thread = thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(mut stream) => {
                        let mut buf = String::new();
                        match stream.read_to_string(&mut buf) {
                            Ok(_) => {}
                            Err(e) => eprintln!("Failed to read socket stream: {}", e)
                        }

                        if buf == "RUN_SYNC" {
                            match sender.send(()) {
                                Ok(()) => {}
                                Err(e) => eprintln!("Failed to signal synchronizer: {}", e)
                            }
                        }
                    },
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        if !run.lock().expect("Unable to acquire run lock").load(Ordering::SeqCst) {
                            break;
                        }
                    }
                    Err(e) => eprintln!("Failed to get stream: {}", e)
                }
            }
        });

        Ok(SocketListener { thread: Some(thread) })
    }

    pub fn join(&mut self) {
        if let Some(thread) = self.thread.take() {
            thread.join().expect("Unable to join thread");
        }
    }
}