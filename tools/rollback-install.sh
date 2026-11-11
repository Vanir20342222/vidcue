#!/usr/bin/env bash
# Restores the previous Vidcue desktop build saved by install.sh as
# ~/.local/bin/vidcue.previous. The current build is kept in its place, so
# rolling forward again is simply running install.sh once more.
set -euo pipefail
bin="${HOME}/.local/bin"
[ -x "$bin/vidcue.previous" ] || { echo "No vidcue.previous to roll back to." >&2; exit 1; }
tmp="$bin/vidcue.new"
cp "$bin/vidcue" "$tmp"
mv -f "$bin/vidcue.previous" "$bin/vidcue"
mv -f "$tmp" "$bin/vidcue.previous"
echo "Rolled back: the previous build is now “vidcue”; the newer one is “vidcue.previous”."
