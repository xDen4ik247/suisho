#!/usr/bin/env bash
# Install / update the runnable copy:  ./install.sh
# App -> ~/.local/share/suisho/app, launcher -> ~/.local/bin/suisho, desktop entry + icon.
# Your workspaces (~/.local/share/suisho/workspaces) and settings (~/.config/suisho) are never touched.
set -euo pipefail
cd "$(dirname "$0")"
DEST="$HOME/.local/share/suisho"
ICONS="$HOME/.local/share/icons/hicolor/scalable/apps"
mkdir -p "$DEST" "$HOME/.local/bin" "$HOME/.local/share/applications" "$ICONS"
command -v cargo >/dev/null || { echo "cargo is needed to build the engine (pacman -S rust)"; exit 1; }
cargo build --release --quiet --manifest-path engine/Cargo.toml
rm -rf "$DEST/app.new"
mkdir -p "$DEST/app.new/bin"
cp -r qml assets "$DEST/app.new/"
rm -f "$DEST/app.new/qml/shaders/"*.frag
install -m 755 engine/target/release/suisho-engine "$DEST/app.new/bin/suisho-engine"
install -m 755 uninstall.sh "$DEST/app.new/uninstall.sh"
# swap in one step (an open window live-reloads; a gap without app/ breaks that reload)
if [ -d "$DEST/app" ] && mv --exchange "$DEST/app.new" "$DEST/app" 2>/dev/null; then
    rm -rf "$DEST/app.new"
else
    rm -rf "$DEST/app" && mv "$DEST/app.new" "$DEST/app"
fi
install -m 755 bin/suisho "$HOME/.local/bin/suisho"
install -m 644 suisho.desktop "$HOME/.local/share/applications/suisho.desktop"
install -m 644 assets/icon/suisho.svg "$ICONS/suisho.svg"
gtk-update-icon-cache -q -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
update-desktop-database -q "$HOME/.local/share/applications" 2>/dev/null || true
echo "installed to $DEST/app"
