#!/bin/sh
# Nightly logical backup of the Vidcue database; keeps the newest 14 dumps.
#
# Off-site copies: when /etc/vidcue/offsite-recipient.pub exists (age recipients, e.g. SSH
# ed25519 public keys, one per line), every dump and the server key are also written
# age-encrypted to /var/backups/vidcue/offsite. A remote machine pulls only those encrypted
# files through a forced SSH command (deploy/vidcue-offsite-send.sh); plaintext never leaves.
set -eu
dir=/var/backups/vidcue
off=$dir/offsite
recipients=/etc/vidcue/offsite-recipient.pub
mkdir -p "$dir"
chmod 700 "$dir"
stamp=$(date +%Y%m%d-%H%M%S)
file="$dir/vidcue-$stamp.dump"
(umask 077; su postgres -c "pg_dump --format=custom vidcue" > "$file.tmp")
mv "$file.tmp" "$file"
ls -1t "$dir"/vidcue-*.dump | tail -n +15 | xargs -r rm --

if [ -s "$recipients" ] && command -v age >/dev/null 2>&1; then
  mkdir -p "$off"
  chmod 700 "$off"
  (umask 077; age -R "$recipients" -o "$off/vidcue-$stamp.dump.age.tmp" "$file")
  mv "$off/vidcue-$stamp.dump.age.tmp" "$off/vidcue-$stamp.dump.age"
  # Re-encrypt the server key whenever it or the recipient list changed.
  if [ ! -s "$off/secret-key.age" ] || [ /etc/vidcue/secret.key -nt "$off/secret-key.age" ] \
    || [ "$recipients" -nt "$off/secret-key.age" ]; then
    (umask 077; age -R "$recipients" -o "$off/secret-key.age.tmp" /etc/vidcue/secret.key)
    mv "$off/secret-key.age.tmp" "$off/secret-key.age"
  fi
  ls -1t "$off"/vidcue-*.dump.age | tail -n +15 | xargs -r rm --
fi
