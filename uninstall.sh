#!/usr/bin/env bash
# uninstall.sh — remove Suisho's program files. Your workspaces (~/.local/share/suisho) and settings
# (~/.config/suisho) stay.
set -uo pipefail
rm -rf "$HOME/.local/share/suisho/app" "$HOME/.local/share/suisho/app.new"
rm -f "$HOME/.local/bin/suisho" "$HOME/.local/share/applications/suisho.desktop" "$HOME/.local/share/icons/hicolor/scalable/apps/suisho.svg"
gtk-update-icon-cache -q -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
update-desktop-database -q "$HOME/.local/share/applications" 2>/dev/null || true
echo "Suisho removed (workspaces in ~/.local/share/suisho kept)"
