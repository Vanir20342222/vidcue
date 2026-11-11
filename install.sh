#!/usr/bin/env bash
# Installs (or updates) the Vidcue desktop app for the current user.
#
# Builds `apps/desktop` in release mode and atomically replaces the `vidcue`
# launcher in ~/.local/bin, keeping the previous binary as `vidcue.previous`
# for tools/rollback-install.sh. Also installs the icon and desktop entry.
#
# The in-app updater (Settings → About → Update now) runs this script from the
# cloned release tag; running it from a checkout does the same job manually.
set -euo pipefail
cd "$(dirname "$0")"

command -v cargo >/dev/null || { echo 'cargo (Rust) is required to install vidcue.' >&2; exit 1; }

echo "Building vidcue (this can take a few minutes)…"
cargo build --release --locked -p vidcue-desktop

bin="${HOME}/.local/bin"
share="${HOME}/.local/share"
mkdir -p "$bin" "$share/applications" "$share/icons/hicolor/scalable/apps"

# Atomically replace the launcher, saving the previous build for rollback.
tmp="$bin/vidcue.new"
cp target/release/vidcue-desktop "$tmp"
chmod 755 "$tmp"
if [ -x "$bin/vidcue" ]; then
    cp "$bin/vidcue" "$bin/vidcue.previous"
fi
mv -f "$tmp" "$bin/vidcue"

install -m 644 apps/desktop/assets/vidcue.svg \
    "$share/icons/hicolor/scalable/apps/dev.vidcue.desktop.svg"
cat > "$share/applications/dev.vidcue.desktop.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Version=1.0
Name=Vidcue
GenericName=Media Player
Comment=Watch, listen and download with Vidcue
Exec=vidcue
Icon=dev.vidcue.desktop
Terminal=false
Categories=AudioVideo;Audio;Video;Player;
Keywords=video;music;youtube;radio;movies;player;
StartupWMClass=vidcue-desktop
EOF

command -v update-desktop-database >/dev/null && update-desktop-database "$share/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -f "$share/icons/hicolor" || true

echo "Installed: $bin/vidcue (version $(grep -m1 '^version' Cargo.toml | cut -d'"' -f2))."
echo "Start it from your app launcher or by running “vidcue”."
