#!/bin/sh
# Serves already-encrypted off-site backup files to a pulling machine. Installed as
# /usr/local/sbin/vidcue-offsite-send and reached only through a forced SSH command:
#
#   command="/usr/local/sbin/vidcue-offsite-proxy",restrict,from="<puller network>" ssh-ed25519 AAAA… puller
#
# where vidcue-offsite-proxy (on the container host) validates SSH_ORIGINAL_COMMAND the same
# way and runs this script inside the container (e.g. `pct exec <id> -- env SSH_ORIGINAL_COMMAND=…`).
#
#   list            names of available encrypted dumps, newest first
#   dump <name>     one encrypted dump
#   secret-key      the encrypted server key
set -eu
off=/var/backups/vidcue/offsite
cmd=${SSH_ORIGINAL_COMMAND:-$*}
case "$cmd" in
  list)
    ls -1t "$off" | grep -E '^vidcue-[0-9]{8}-[0-9]{6}\.dump\.age$' || true
    ;;
  "dump "*)
    name=${cmd#dump }
    echo "$name" | grep -qE '^vidcue-[0-9]{8}-[0-9]{6}\.dump\.age$' || { echo "bad name" >&2; exit 2; }
    cat "$off/$name"
    ;;
  secret-key)
    cat "$off/secret-key.age"
    ;;
  *)
    echo "usage: list | dump <name> | secret-key" >&2
    exit 2
    ;;
esac
