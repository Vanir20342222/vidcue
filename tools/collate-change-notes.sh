#!/usr/bin/env bash
# Print the change notes dated on or after the given day (default: all), oldest first, for
# folding into the plan, parity and validation documents at release time.
#   usage: tools/collate-change-notes.sh [YYYY-MM-DD]
set -euo pipefail
since="${1:-0000-00-00}"
[[ "$since" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]] || { echo 'usage: collate-change-notes.sh [YYYY-MM-DD]' >&2; exit 2; }
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
shopt -s nullglob
for note in "$root"/docs/changes/20[0-9][0-9]-[0-9][0-9]-[0-9][0-9]-*.md; do
    name="$(basename "$note")"
    [[ "${name:0:10}" < "$since" ]] && continue
    printf '\n<!-- %s -->\n' "$name"
    cat "$note"
done
