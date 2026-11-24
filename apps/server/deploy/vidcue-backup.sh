#!/bin/sh
# Nightly logical backup of the Vidcue database; keeps the newest 14 dumps.
set -eu
dir=/var/backups/vidcue
mkdir -p "$dir"
chmod 700 "$dir"
file="$dir/vidcue-$(date +%Y%m%d-%H%M%S).dump"
(umask 077; su postgres -c "pg_dump --format=custom vidcue" > "$file.tmp")
mv "$file.tmp" "$file"
ls -1t "$dir"/vidcue-*.dump | tail -n +15 | xargs -r rm --
