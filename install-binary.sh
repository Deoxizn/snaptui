#!/usr/bin/env bash
# snaptui installer (prebuilt binary) — everything install.sh does except the cargo build.
#
# 1. Installs the bundled binary to ~/.local/bin/snaptui,
#    .desktop file to ~/.local/share/applications/,
#    and icon to ~/.local/share/icons/hicolor/*/apps/.
# 2. Grants your user access to the snapper D-Bus API by setting
#    ALLOW_USERS on every snapper config (asks for your sudo password once).
#
# Safe to run twice. No cargo / Rust toolchain needed.

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"

BIN_SRC="${SCRIPT_DIR}/snaptui"
BIN="${HOME}/.local/bin/snaptui"
[[ -f $BIN_SRC ]] || { echo "!! snaptui binary not found next to install-binary.sh, aborting" >&2; exit 1; }
mkdir -p "$(dirname "$BIN")"
cp "$BIN_SRC" "$BIN"
chmod +x "$BIN"
echo ">> installed ${BIN}"

# Icon — prefer the source tree copy, fall back to the bundled one next to this script
ICON_DST_DIR="${HOME}/.local/share/icons/hicolor/512x512/apps"
ICON_DST="${ICON_DST_DIR}/snaptui.png"
ICON_SRC=""
if [[ -f "${HOME}/Work/snaptui/icon.png" ]]; then
    ICON_SRC="${HOME}/Work/snaptui/icon.png"
elif [[ -f "${SCRIPT_DIR}/icon.png" ]]; then
    ICON_SRC="${SCRIPT_DIR}/icon.png"
fi
if [[ -n $ICON_SRC ]]; then
    mkdir -p "$ICON_DST_DIR"
    cp "$ICON_SRC" "$ICON_DST"
    echo ">> icon installed to ${ICON_DST} (from ${ICON_SRC})"
    # refresh icon cache if tool exists (best-effort, never fails install)
    gtk-update-icon-cache -q "${HOME}/.local/share/icons/hicolor" 2>/dev/null || true
else
    echo "!! icon.png not found in ~/Work/snaptui or next to install-binary.sh, skipping icon install"
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
mapfile -t configs < <(snapper list-configs 2>/dev/null | awk 'NR > 2 { print $1 }') || true
if [[ "${#configs[@]}" -eq 0 ]]; then
    echo "!! no snapper configs found — nothing to grant"
else
    for cfg in "${configs[@]}"; do
        echo "   - ${cfg}: ALLOW_USERS=\"${USER}\""
        sudo snapper -c "$cfg" set-config "ALLOW_USERS=${USER}"
    done
fi

echo ""
echo ">> done. run 'snaptui' to launch."
