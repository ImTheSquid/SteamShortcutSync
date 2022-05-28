#!/bin/sh

cargo install steam_shortcut_sync
cargo install steam_shortcut_sync_client

BASEDIR=$(dirname $0)
cp $BASEDIR/SteamShortcutSync.service $HOME/.local/share/systemd/user/
systemctl --user enable SteamShortcutSync.service
systemctl --user start SteamShortcutSync.service