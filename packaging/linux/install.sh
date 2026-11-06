#!/bin/sh
# Installs Flight Replay from this folder. Usage: ./install.sh [PREFIX]   (default: ~/.local)
# Remove it again with: ./install.sh --uninstall [PREFIX]
set -eu

here=$(cd "$(dirname "$0")" && pwd)
action=install
if [ "${1:-}" = "--uninstall" ]; then
    action=uninstall
    shift
fi
prefix=${1:-"$HOME/.local"}

if [ "$action" = uninstall ]; then
    rm -f "$prefix/bin/flight-replay" "$prefix/share/applications/flight-replay.desktop"
    find "$prefix/share/icons/hicolor" -name 'flight-replay.*' -delete 2>/dev/null || true
    echo "Flight Replay was removed from $prefix"
    exit 0
fi

install -d "$prefix/bin" "$prefix/share/applications"
install -m 755 "$here/flight-replay" "$prefix/bin/flight-replay"
install -m 644 "$here/share/applications/flight-replay.desktop" "$prefix/share/applications/flight-replay.desktop"
(cd "$here/share/icons" && find hicolor -type f) | while read -r icon; do
    install -d "$prefix/share/icons/$(dirname "$icon")"
    install -m 644 "$here/share/icons/$icon" "$prefix/share/icons/$icon"
done
echo "Flight Replay was installed in $prefix"
echo "It needs ffmpeg and ffprobe: see README.md."
