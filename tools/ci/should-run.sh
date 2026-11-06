#!/usr/bin/env bash
# Print "true" or "false": does the change under test need the named heavy CI area?
#   usage: tools/ci/should-run.sh android|rust [event_name]
# The change under test is HEAD against its first parent. That is the PR's own change for a
# pull_request merge ref and for a merge-queue entry. Anything that is not a pull request or a
# merge-queue entry (push to main, schedule, manual dispatch) always runs everything, so release
# gating on main never sees a skipped build.
# A path that matches no skip pattern runs the area, so an unknown file fails safe.
set -euo pipefail
area="${1:?area}"
event="${2:-${GITHUB_EVENT_NAME:-}}"
case "$event" in pull_request|merge_group) ;; *) echo true; exit 0;; esac

# Never need either area: documentation, templates and unrelated workflows.
common='^docs/|^[^/]+\.md$|^LICENSE$|^\.github/pull_request_template\.md$|^\.gitleaksignore$|^\.github/workflows/(pr-hygiene|security|release-candidate|youtube-live|native-playback)\.yml$'
case "$area" in
    # Android builds vidcue-core, vidcue-providers, vidcue-mobile and vendor/. The desktop crates do not reach it.
    android) skip="$common|^apps/desktop/|^crates/vidcue-services/|^examples/|^packaging/|^install(-rust)?\.sh$|^\.github/workflows/rust\.yml$|^tools/(test-|package-release|rollback-install|native-playback-urls|license_bundle)" ;;
    # The Rust workspace does not build the Kotlin app or the Android scripts.
    rust) skip="$common|^apps/android/|^tools/android/|^\.github/workflows/android[^/]*\.yml$" ;;
    *) echo "unknown area: $area" >&2; exit 2 ;;
esac

changed="$(git diff --name-only HEAD^1 HEAD)"
[[ -n "$changed" ]] || { echo true; exit 0; }
if grep -Evq "$skip" <<<"$changed"; then echo true; else echo false; fi
