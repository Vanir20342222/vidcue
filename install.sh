#!/usr/bin/env bash
# Build/install the Rust app. Existing settings, media and legacy data stay intact.
set -euo pipefail
if ! command -v cargo >/dev/null 2>&1; then
    printf 'Install current stable Rust first: https://rustup.rs\n' >&2
    exit 1
fi
vidcue_source_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
vidcue_source_tmp=''
vidcue_publish_tmp=''
cleanup() {
    if [[ -n "$vidcue_source_tmp" ]]; then rm -rf -- "$vidcue_source_tmp"; fi
    if [[ -n "$vidcue_publish_tmp" ]]; then rm -rf -- "$vidcue_publish_tmp"; fi
}
trap cleanup EXIT
if [[ ! -f "$vidcue_source_dir/Cargo.toml" || ! -d "$vidcue_source_dir/apps/desktop" ]]; then
    command -v git >/dev/null 2>&1 || { printf 'Install git first.\n' >&2; exit 1; }
    vidcue_source_tmp="$(mktemp -d "${TMPDIR:-/tmp}/vidcue-source.XXXXXX")"
    git clone --depth 1 --branch "${YTUI_BRANCH:-main}" \
        https://github.com/Vanir20342222/ytui.git "$vidcue_source_tmp/repo"
    vidcue_source_dir="$vidcue_source_tmp/repo"
fi
vidcue_install_dir="${YTUI_INSTALL_DIR:-$HOME/.local/bin}"
vidcue_build_dir="${CARGO_TARGET_DIR:-$vidcue_source_dir/target}"
cargo build --manifest-path "$vidcue_source_dir/Cargo.toml" --target-dir "$vidcue_build_dir" --release --locked -p vidcue
mkdir -p -- "$vidcue_install_dir"
vidcue_publish_tmp="$(mktemp -d "$vidcue_install_dir/.vidcue-install.XXXXXX")"
install -m 755 -- "$vidcue_build_dir/release/vidcue" "$vidcue_publish_tmp/vidcue"
"$vidcue_publish_tmp/vidcue" --version
# vidcue is the command; ytui and ytui-rs keep working for scripts and habits from before the rename.
for vidcue_launcher in vidcue ytui ytui-rs vidcue.previous; do
    [[ ! -d "$vidcue_install_dir/$vidcue_launcher" ]] || { printf 'Installation target is a directory: %s\n' "$vidcue_install_dir/$vidcue_launcher" >&2; exit 1; }
done
# Snapshot the executable being replaced: vidcue, or ytui when upgrading from before the rename.
for vidcue_old in vidcue ytui; do
    if [[ -f "$vidcue_install_dir/$vidcue_old" && ! -L "$vidcue_install_dir/$vidcue_old" ]]; then
        if ! cmp -s -- "$vidcue_install_dir/$vidcue_old" "$vidcue_publish_tmp/vidcue"; then
            install -m755 -- "$vidcue_install_dir/$vidcue_old" "$vidcue_publish_tmp/vidcue.previous"
            mv -fT -- "$vidcue_publish_tmp/vidcue.previous" "$vidcue_install_dir/vidcue.previous"
        fi
        break
    fi
done
ln -- "$vidcue_publish_tmp/vidcue" "$vidcue_publish_tmp/ytui"
ln -- "$vidcue_publish_tmp/vidcue" "$vidcue_publish_tmp/ytui-rs"
# Replace an old launcher/symlink itself, never write through it into a virtualenv.
mv -fT -- "$vidcue_publish_tmp/vidcue" "$vidcue_install_dir/vidcue"
mv -fT -- "$vidcue_publish_tmp/ytui" "$vidcue_install_dir/ytui"
mv -fT -- "$vidcue_publish_tmp/ytui-rs" "$vidcue_install_dir/ytui-rs"
printf 'Installed %s/vidcue (old ytui and ytui-rs commands still work).\nRun vidcue doctor, then vidcue.\n' "$vidcue_install_dir"
printf 'Restart running vidcue processes to use this build. Previous changed executable: vidcue.previous (when available).\n'
