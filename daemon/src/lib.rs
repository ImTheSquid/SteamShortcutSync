use std::{sync::{atomic::{AtomicBool, Ordering}, mpsc::{self, TryRecvError}, Arc}, thread, os::unix::net::UnixListener, env::{self, VarError}, path::{Path, PathBuf}, io::{self, Read, Error}, process::{self, Command}, time::Duration, fs, collections::HashMap};
use lazy_static::lazy_static;

use notify::Watcher;
use regex::Regex;
use walkdir::WalkDir;

pub struct Synchronizer {
    thread: Option<thread::JoinHandle<()>>
}

enum SynchronizerChildCommand {
    Run,
    Die
}

pub struct SynchronizerCreationError {
    pub kind: SynchronizerCreationErrorKind
}

#[derive(Debug, Clone)]
pub enum SynchronizerCreationErrorKind {
    NoHomeDir,
    NoApplicationsDir
}

#[derive(Hash, PartialEq, Eq)]
struct SteamDesktopFile {
    name: String,
    id: String,
}

impl Synchronizer {
    pub fn new(receiver: mpsc::Receiver<()>, run: Arc<AtomicBool>) -> Result<Synchronizer, SynchronizerCreationError> {
        let desktop_path = match Self::steam_dir() {
            Ok(val) => val,
            Err(_) => return Err(SynchronizerCreationError { kind: SynchronizerCreationErrorKind::NoHomeDir })
        };

        if !desktop_path.is_dir() {
            match fs::create_dir_all(&desktop_path) {
                Ok(_) => {},
                Err(_) => return Err(SynchronizerCreationError { kind: SynchronizerCreationErrorKind::NoApplicationsDir })
            };
        }

        let thread = thread::spawn(move || {
            let (tx, rx) = mpsc::channel();

            let working = Arc::new(AtomicBool::new(false));
            let w = Arc::clone(&working);

            // Worker thread to actually synchronize
            let worker = thread::spawn(move || loop {
                match rx.recv() {
                    Ok(command) => match command {
                        SynchronizerChildCommand::Run => {
                            w.store(true, Ordering::SeqCst);
                            Self::synchronize();
                            w.store(false, Ordering::SeqCst);
                        },
                        SynchronizerChildCommand::Die => break
                    },
                    Err(_) => {
                        eprintln!("Failed to communicate with parent synchronizer thread");
                        process::exit(7);
                    }
                }
            });

            // Makes sure multiple synchronizations aren't run at the same time
            loop {
                match receiver.try_recv() {
                    Ok(_) => {
                        if working.load(Ordering::SeqCst) {
                            continue;
                        }

                        tx.send(SynchronizerChildCommand::Run).expect("Unable to send command to child synchronizer thread");
                    },
                    Err(e) => match e {
                        TryRecvError::Disconnected => {
                            eprintln!("Failed to watch for receiver: {}", e);
                            run.store(false, Ordering::SeqCst);
                            break;
                        },
                        TryRecvError::Empty => {
                            if !run.load(Ordering::SeqCst) {
                                tx.send(SynchronizerChildCommand::Die).expect("Unable to send command to child synchronizer thread");
                                worker.join().expect("Unable to join worker thread");
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(Synchronizer { thread: Some(thread) })
    }

    pub fn join(&mut self) {
        if let Some(thread) = self.thread.take() {
            thread.join().expect("Unable to join thread");
        }
    }

    fn steam_dir() -> Result<PathBuf, VarError> {
        let key = env::var("HOME")?;

        Ok(Path::new(&key).join(".var/app/com.valvesoftware.Steam"))
    }

    fn desktop_dir() -> Result<PathBuf, VarError> {
        let key = env::var("HOME")?;

        Ok(Path::new(&key).join(".local/share/applications"))
    }

    fn icons_dir() -> Result<PathBuf, VarError> {
        let key = env::var("HOME")?;

        Ok(Path::new(&key).join(".icons"))
    }

    fn icon_from_id(id: &str, root: &PathBuf) -> Option<PathBuf> {
        let icon_filename = format!("steam_icon_{}.png", id);

        // Recursively search for icon
        let mut entries: Vec<_> = WalkDir::new(root)
            .into_iter()
            .filter_map(|f| f.ok())
            .filter(|f| f.file_name().to_str().unwrap_or("") == icon_filename)
            .filter_map(|path| {
                if let Some(s) = path.into_path().to_str() {
                    return Some(String::from(s));
                }
                None
            })
            .collect();

        if entries.len() == 1 {
            return Some(Path::new(&entries.pop().unwrap()).to_path_buf())
        } else if entries.len() == 0 {
            return None
        }

        // Find highest-res icon
        entries.sort();

        Some(Path::new(&entries.pop().unwrap()).to_path_buf())
    }

    fn load_desktop_files(src: &PathBuf) -> Vec<SteamDesktopFile> {
        lazy_static! {
            static ref EXEC_REGEX: Regex = Regex::new("^Exec=.*steam://rungameid/[0-9]+$").unwrap();
            static ref NAME_REGEX: Regex = Regex::new("^Name=.+$").unwrap();
        };

        WalkDir::new(src)
            .into_iter()
            .filter_map(|f| f.ok())
            .filter_map(|f| fs::read_to_string(f.path()).ok())
            .filter_map(|contents| {
                let mut name = "";
                let mut exec = "";
                for line in contents.lines() {
                    if EXEC_REGEX.is_match(line) {
                        exec = line;
                    } else if NAME_REGEX.is_match(line) {
                        name = line;
                    }
                }
                
                if exec.len() > 0 && name.len() > 0 {
                    let last_slash = exec.rfind('/').unwrap();

                    Some(SteamDesktopFile {
                        name: name[5..].to_string(),
                        id: exec[last_slash + 1..].parse().unwrap()
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    fn write_desktop_file(file: &SteamDesktopFile, dest_dir: &PathBuf) -> Result<(), Error> {
        let contents = format!("{}\n{}{}\n{}{}\n{}\n{}{}",
            "[Desktop Entry]",
            "Name=",
            file.name,
            "Icon=steam_icon_",
            file.id,
            "Type=Application",
            "Exec=xdg-open steam://rungameid/",
            file.id
        );

        fs::write(dest_dir.join(format!("{}.desktop", file.name)), contents)
    }

    fn synchronize() {
        println!("Starting Synchronization");
        // Load indexed vs steam to see what needs to be added or removed
        let steam_path = Self::steam_dir().expect("Failed to load steam desktop dir files");
        let steam = Self::load_desktop_files(&steam_path.join("data/applications"));
        let desktop_path = Self::desktop_dir().expect("Failed to load desktop dir files");
        let icons_path = Self::icons_dir().expect("Failed to find icons dir");
        let indexed = Self::load_desktop_files(&desktop_path);

        let mut map = HashMap::new();

        for desktop in steam.into_iter() {
            map.insert(desktop, 1);
        }

        for desktop in indexed.into_iter() {
            if let Some(&cnt) = map.get(&desktop) {
                map.insert(desktop, cnt - 1);
            } else {
                map.insert(desktop, -1);
            }
        }

        // Remove old entries/icons and convert new entries/icons
        for (desktop_file, cnt) in map.into_iter() {
            if cnt >= 1 {
                println!("Adding desktop file {}", desktop_file.name);
                match Self::write_desktop_file(&desktop_file, &desktop_path) {
                    Ok(_) => {},
                    Err(e) => {
                        eprintln!("Unable to write desktop file {}: {}", &desktop_file.name, e);
                        continue;
                    }
                };

                // Write icon
                if let Some(path) = Self::icon_from_id(&desktop_file.id, &steam_path.join("data/icons/hicolor")) {
                    match fs::create_dir_all(&icons_path){
                        Ok(_) => {},
                        Err(e) => {
                            eprintln!("Failed to create icons directory at {}: {}", icons_path.to_str().unwrap_or("ERROR!"), e);
                            continue;
                        }
                    }

                    match fs::copy(&path, &icons_path.join(&path.file_name().unwrap())) {
                        Ok(_) => {},
                        Err(e) => eprintln!("Unable to write icon for desktop file {}: {}", desktop_file.name, e)
                    }
                } else {
                    eprintln!("No icon found for desktop file {}", desktop_file.name);
                }
            } else if cnt <= -1 { // Remove file
                println!("Removing desktop file {}", desktop_file.name);
                match fs::remove_file(desktop_path.join(format!("{}.desktop", desktop_file.name))) {
                    Ok(_) => {},
                    Err(e) => eprintln!("Unable to remove desktop file {}: {}", desktop_file.name, e)
                };

                // Remove icon file if possible
                if let Some(path) = Self::icon_from_id(&desktop_file.id, &Path::new("/usr/share/pixmaps").to_path_buf()) {
                    match fs::remove_file(path) {
                        Ok(_) => {},
                        Err(e) => eprintln!("Unable to remove icon for desktop file {}: {}", desktop_file.name, e)
                    }
                }
            } else {
                println!("Skipping desktop file {}", desktop_file.name);
            }
        }

        // Update
        Command::new("update-desktop-database").arg(&desktop_path).status().expect("Failed to update desktop database");

        println!("Synchronization Complete");
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
    NoApplicationsDir,
    UnableToWatch
}

impl FileChangeListener {
    pub fn new(sender: mpsc::Sender<()>, run: Arc<AtomicBool>) -> Result<FileChangeListener, FileChangeListenerCreationError> {
        // Currently only looks at Flatpak directory
        let key = match env::var("HOME") {
            Ok(val) => val,
            Err(_) => return Err(FileChangeListenerCreationError { kind: FileChangeListenerCreationErrorKind::NoHomeDir })
        };

        let steam_path = Path::new(&key).join(".var/app/com.valvesoftware.Steam/data/applications");

        if !steam_path.is_dir() {
            return Err(FileChangeListenerCreationError { kind: FileChangeListenerCreationErrorKind::NoSteamDir });
        }

        let (tx, rx) = mpsc::channel();

        let mut watcher = match notify::watcher(tx, Duration::from_secs(10)) {
            Ok(w) => w,
            Err(_) => return Err(FileChangeListenerCreationError { kind: FileChangeListenerCreationErrorKind::UnableToWatch })
        };

        match watcher.watch(&steam_path, notify::RecursiveMode::Recursive) {
            Ok(_) => {},
            Err(_) => return Err(FileChangeListenerCreationError { kind: FileChangeListenerCreationErrorKind::UnableToWatch })
        }

        let thread = thread::spawn(move || {
            let mut watcher = watcher;
            loop {
                match rx.try_recv() {
                    Ok(_) => match sender.send(()) {
                        Ok(()) => println!("File change detected, sync request sent."),
                        Err(e) => eprintln!("Failed to signal synchronizer from FileChangeListener: {}", e)
                    },
                    Err(e) => match e {
                        TryRecvError::Disconnected => {
                            eprintln!("File watcher event failed: {}", e);
                            process::exit(8);
                        },
                        TryRecvError::Empty => {
                            if !run.load(Ordering::SeqCst) {
                                match watcher.unwatch(&steam_path) {
                                    Ok(_) => println!("Unwatched steam dir"),
                                    Err(e) => {
                                        eprintln!("Failed to unwatch steam path: {}", e);
                                        process::exit(5);
                                    }
                                }

                                break;
                            }
                        }
                    }
                }
            }
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
    pub fn new(sender: mpsc::Sender<()>, run: Arc<AtomicBool>) -> Result<SocketListener, SocketListenerCreationError> {
        // Attempt to load env var
        let key = match env::var("XDG_RUNTIME_DIR") {
            Ok(val) => val,
            Err(_) => return Err(SocketListenerCreationError { kind: SocketListenerCreationErrorKind::NoRuntimeDir })
        };
    
        let path = Path::new(&key).join("steam-shortcut-sync.sock");

        // Create a socket listener on a separate thread
        let listener = match UnixListener::bind(&path) {
            Ok(sock) => sock,
            Err(e) => {
                eprintln!("Failed to create listener: {}", e);
                return Err(SocketListenerCreationError { kind: SocketListenerCreationErrorKind::NoSocket });
            }
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
                                Ok(()) => println!("Manual sync request received, sync request sent."),
                                Err(e) => eprintln!("Failed to signal synchronizer: {}", e)
                            }
                        }
                    },
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        if !run.load(Ordering::SeqCst) {
                            match fs::remove_file(&path) {
                                Ok(()) => println!("Cleaned up socket"),
                                Err(e) => {
                                    eprintln!("Failed to remove socket: {}", e);
                                    process::exit(6);
                                }
                            }
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