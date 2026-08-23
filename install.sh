#!/usr/bin/env bash
# snaptui installer
#
# 1. Builds the release binary.
# 2. Installs it to ~/.local/bin/snaptui.
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

APP_DIR="${HOME}/.local/share/applications"
mkdir -p "$APP_DIR"
DESKTOP="${APP_DIR}/snaptui.desktop"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cat > "$DESKTOP" <<EOF
[Desktop Entry]
Name=SnaptUI
Comment=Btrfs snapshot manager TUI
Exec=omarchy-launch-or-focus-tui --app-id=TUI.snaptui ${BIN}
Icon=${SCRIPT_DIR}/icon.png
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
