#!/usr/bin/env bash
# snaptui installer — everything lands in ~/.local
#
# 1. Builds the release binary.
# 2. Installs binary to ~/.local/bin/snaptui,
#    .desktop file to ~/.local/share/applications/,
#    and icon to ~/.local/share/icons/hicolor/*/apps/.
# 3. Grants your user access to the snapper D-Bus API by setting
#    ALLOW_USERS on every snapper config (asks for your sudo password once).

set -euo pipefail

cd "$(dirname "$0")"

echo ">> building snaptui (release)..."
cargo build --release

BIN="${HOME}/.local/bin/snaptui"
mkdir -p "$(dirname "$BIN")"
cp target/release/snaptui "$BIN"
echo ">> installed ${BIN}"

# Icon — installed to ~/.local so the app is self-contained after the source tree is deleted
ICON_SRC="$(dirname "$0")/icon.png"
ICON_DIR="${HOME}/.local/share/icons/hicolor/512x512/apps"
ICON_DST="${ICON_DIR}/snaptui.png"
if [ -f "$ICON_SRC" ]; then
    mkdir -p "$ICON_DIR"
    cp "$ICON_SRC" "$ICON_DST"
    echo ">> icon installed to ${ICON_DST}"
    # refresh icon cache if tool exists (best-effort, never fails install)
    gtk-update-icon-cache -q "${HOME}/.local/share/icons/hicolor" 2>/dev/null || true
else
    echo "!! icon.png not found next to install.sh, skipping icon install"
    ICON_DST=""
fi

APP_DIR="${HOME}/.local/share/applications"
mkdir -p "$APP_DIR"
DESKTOP="${APP_DIR}/snaptui.desktop"
cat > "$DESKTOP" <<EOF
[Desktop Entry]
Name=SnaptUI
Comment=Btrfs snapshot manager TUI
Exec=omarchy-launch-or-focus-tui --app-id=TUI.snaptui ${BIN}
Icon=snaptui
Terminal=false
Type=Application
Categories=System;
Keywords=btrfs;snapshot;snapper;
EOF
echo ">> desktop file installed to ${DESKTOP}"

echo ">> granting D-Bus access (ALLOW_USERS) for all snapper configs..."
mapfile -t configs < <(snapper list-configs | awk 'NR > 2 { print $1 }')
if [ "${#configs[@]}" -eq 0 ]; then
    echo "!! no snapper configs found — nothing to grant"
else
    for cfg in "${configs[@]}"; do
        echo "   - ${cfg}: ALLOW_USERS=\"${USER}\""
        sudo snapper -c "$cfg" set-config "ALLOW_USERS=${USER}"
    done
fi

echo ""
echo ">> done. run 'snaptui' to launch."
