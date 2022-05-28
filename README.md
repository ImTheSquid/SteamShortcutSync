# Steam Shortcut Sync
A client and daemon to automatically synchronize shortcuts created by the Flatpak version of Steam to a place where most launchers are able to index and use them.

## Activation
Shortcut synchronization can be activated either automaitcally by a change in Steam's internal shortcuts directory or manually by running the client. The daemon must be running in order for either of these events to be processed.

## Crates.io
- Client: [`steam-shortcut-sync-client`](https://crates.io/crates/steam-shortcut-sync-client)
- Daemon: [`steam-shortcut-sync`](https://crates.io/crates/steam-shortcut-sync)

Note: Installing these directly doesn't enable automatic daemon startup.

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

If you want to be able to run the client from anywhere, add `$HOME/.cargo/bin` to your `PATH`.

### Updating
To update, run the install script again.