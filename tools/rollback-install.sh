#!/usr/bin/env bash
# Explicit executable-only rollback. Does not touch configuration or state.
set -euo pipefail
rollback_bin="${YTUI_INSTALL_DIR:-$HOME/.local/bin}"
[[ -f "$rollback_bin/vidcue.previous" && ! -L "$rollback_bin/vidcue.previous" ]] || { echo 'No previous executable snapshot exists.' >&2; exit 1; }
for rollback_name in vidcue ytui ytui-rs; do
  [[ ! -d "$rollback_bin/$rollback_name" ]] || { echo 'A launcher path is a directory.' >&2; exit 1; }
done
rollback_stage="$(mktemp -d "$rollback_bin/.vidcue-rollback.XXXXXX")"
trap 'rm -rf -- "$rollback_stage"' EXIT
install -m755 -- "$rollback_bin/vidcue.previous" "$rollback_stage/vidcue"
"$rollback_stage/vidcue" --version
ln -- "$rollback_stage/vidcue" "$rollback_stage/ytui"
ln -- "$rollback_stage/vidcue" "$rollback_stage/ytui-rs"
# Keep the newer executable for an explicit roll-forward.
if [[ -f "$rollback_bin/vidcue" && ! -L "$rollback_bin/vidcue" ]]; then install -m755 -- "$rollback_bin/vidcue" "$rollback_stage/next"; fi
mv -fT -- "$rollback_stage/vidcue" "$rollback_bin/vidcue"
mv -fT -- "$rollback_stage/ytui" "$rollback_bin/ytui"
mv -fT -- "$rollback_stage/ytui-rs" "$rollback_bin/ytui-rs"
if [[ -f "$rollback_stage/next" ]]; then mv -fT -- "$rollback_stage/next" "$rollback_bin/vidcue.previous"; fi
printf 'Executable rollback complete. Restart vidcue. Existing state was not migrated or replaced.\n'
