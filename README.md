# Steam Shortcut Sync
A client and daemon to automatically synchronize shortcuts created by the Flatpak version of Steam to a place where most launchers are able to index and use them.

## Activation
Shortcut synchronization can be activated either automaitcally by a change in Steam's internal shortcuts directory or manually by running the client. The daemon must be running in order for either of these events to be processed.

## Installation
### Prerequisites
- Systemd
- Rust
- Cargo
### Manual
Run:
```
./install.sh
```