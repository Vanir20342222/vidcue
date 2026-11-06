#!/usr/bin/env bash
# Offline check of tools/ci/should-run.sh against a throwaway repository.
set -euo pipefail
script="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/should-run.sh"
work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT
cd "$work"
git init -q . && git config user.email t@example.invalid && git config user.name t
git commit -q --allow-empty -m base
failures=0
expect() { # expect <android> <rust> <path>...
    local want_android="$1" want_rust="$2"; shift 2
    git checkout -q --detach "$(git rev-list --max-parents=0 HEAD | head -n 1)"
    local path
    for path in "$@"; do mkdir -p "$(dirname "$path")"; echo "$RANDOM" > "$path"; done
    git add -A && git commit -q -m change
    local got_android got_rust
    got_android="$("$script" android pull_request)"
    got_rust="$("$script" rust merge_group)"
    if [[ "$got_android" != "$want_android" || "$got_rust" != "$want_rust" ]]; then
        echo "FAIL [$*]: android=$got_android (want $want_android) rust=$got_rust (want $want_rust)" >&2
        failures=$((failures + 1))
    fi
}
expect false false docs/android/VALIDATION.md docs/changes/2026-11-03-x.md README.md
expect false true  apps/desktop/src/app.rs crates/vidcue-services/src/lib.rs
expect true  false apps/android/app/src/main/java/dev/ytui/android/ComposeHost.kt tools/android/build.sh
expect true  true  crates/vidcue-providers/src/lib.rs
expect true  true  Cargo.lock
expect true  true  vendor/yt-dlp-rs/src/lib.rs
expect true  true  tools/ci-install-media.sh
expect true  true  some/new/unknown/file.bin
expect true  false .github/workflows/android.yml
expect false true  .github/workflows/rust.yml
# Pushes, schedules and manual runs always run everything.
[[ "$("$script" android push)" == true && "$("$script" rust schedule)" == true && "$("$script" android workflow_dispatch)" == true ]] ||
    { echo 'FAIL: non-PR events must always run' >&2; failures=$((failures + 1)); }
(( failures == 0 )) && echo 'should-run.sh: all checks passed'
exit "$failures"
