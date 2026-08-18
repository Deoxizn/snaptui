#!/usr/bin/env bash
# snaptui uninstaller
#
# Reverses install.sh:
#   1. Removes ~/.local/bin/snaptui
#   2. Removes the .desktop file from ~/.local/share/applications/
#   3. Clears ALLOW_USERS on snapper configs — but ONLY where it is
#      currently set to your username, so a value you set yourself (or
#      that another tool, e.g. btrfs-assistant, relies on) is left alone.
#
# Usage: ./uninstall.sh

set -euo pipefail

cd "$(dirname "$0")"

BIN="${HOME}/.local/bin/snaptui"

echo ">> removing installed binary..."
if [ -f "$BIN" ]; then
    rm -f "$BIN"
    echo "   removed ${BIN}"
else
    echo "   ${BIN} not found, skipping"
fi

DESKTOP="${HOME}/.local/share/applications/snaptui.desktop"
echo ">> removing desktop file..."
if [ -f "$DESKTOP" ]; then
    rm -f "$DESKTOP"
    echo "   removed ${DESKTOP}"
else
    echo "   ${DESKTOP} not found, skipping"
fi

echo ">> clearing ALLOW_USERS granted by install.sh..."
mapfile -t configs < <(snapper list-configs | awk 'NR > 2 { print $1 }')
if [ "${#configs[@]}" -eq 0 ]; then
    echo "   no snapper configs found"
else
    cleared=0
    for cfg in "${configs[@]}"; do
        current="$(sudo snapper -c "$cfg" get-config ALLOW_USERS 2>/dev/null || true)"
        current="${current#ALLOW_USERS=}"
        current="${current#\"}"
        current="${current%\"}"
        if [ "$current" = "$USER" ]; then
            sudo snapper -c "$cfg" set-config "ALLOW_USERS="
            echo "   - ${cfg}: ALLOW_USERS cleared"
            cleared=1
        else
            echo "   - ${cfg}: ALLOW_USERS untouched (was '${current}')"
        fi
    done
    [ "$cleared" -eq 1 ] || echo "   nothing matched — nothing to clear"
fi

echo ""
echo ">> done."
echo "   To remove the source tree too: rm -rf \"${PWD}\""
