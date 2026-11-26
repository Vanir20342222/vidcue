#!/usr/bin/env bash
# Installs a downloaded Vidcue Linux build for the current user, without building anything.
#
# The binary goes to ~/.local/lib/vidcue/vidcue and ~/.local/bin/vidcue links to it, so the
# in-app updater offers the release page (it rebuilds from source only for the install.sh
# launcher). The previous binary is kept as vidcue.previous. Run it from the unpacked folder.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
[ -x "$here/vidcue" ] || { echo "Run this from the unpacked vidcue folder." >&2; exit 1; }

lib="${HOME}/.local/lib/vidcue"
bin="${HOME}/.local/bin"
share="${HOME}/.local/share"
mkdir -p "$lib" "$bin" "$share/applications" "$share/icons/hicolor/scalable/apps"

if [ -x "$lib/vidcue" ]; then
    cp "$lib/vidcue" "$lib/vidcue.previous"
fi
install -m 755 "$here/vidcue" "$lib/vidcue.new"
mv -f "$lib/vidcue.new" "$lib/vidcue"

# An install.sh launcher at ~/.local/bin/vidcue is a real file: keep it as a rollback copy.
if [ -f "$bin/vidcue" ] && [ ! -L "$bin/vidcue" ]; then
    mv -f "$bin/vidcue" "$bin/vidcue.source-build"
fi
ln -sfn "$lib/vidcue" "$bin/vidcue"

install -m 644 "$here/vidcue.svg" "$share/icons/hicolor/scalable/apps/dev.vidcue.desktop.svg"
cat > "$share/applications/dev.vidcue.desktop.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Version=1.0
Name=Vidcue
GenericName=Media Player
Comment=Watch, listen and download with Vidcue
Exec=$bin/vidcue
Icon=dev.vidcue.desktop
Terminal=false
Categories=AudioVideo;Audio;Video;Player;
Keywords=video;music;youtube;radio;movies;player;
StartupWMClass=vidcue-desktop
DESKTOP

command -v update-desktop-database >/dev/null && update-desktop-database "$share/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -f "$share/icons/hicolor" || true

# Report missing runtime libraries instead of failing silently at the first start.
if command -v ldd >/dev/null; then
    missing="$(ldd "$lib/vidcue" 2>/dev/null | awk '/not found/ {print $1}')"
    if [ -n "$missing" ]; then
        echo "Installed, but these libraries are missing:" >&2
        echo "$missing" | sed 's/^/  /' >&2
        echo "Install mpv (libmpv), webkit2gtk-4.1 and libxkbcommon with your package manager." >&2
        exit 0
    fi
fi
echo "Installed: $bin/vidcue → $lib/vidcue"
echo "Start it from your app launcher or by running “vidcue”."
