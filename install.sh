#!/bin/sh

cargo install steam-shortcut-sync
cargo install steam-shortcut-sync-client

BASEDIR=$(dirname $0)
mkdir -p $HOME/.local/share/systemd/user/
cp $BASEDIR/SteamShortcutSync.service $HOME/.local/share/systemd/user/SteamShortcutSync.service
systemctl --user enable SteamShortcutSync.service
systemctl --user start SteamShortcutSync.service