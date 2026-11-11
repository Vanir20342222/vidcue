#!/usr/bin/env bash
# GitHub-hosted Ubuntu 24.04 x64 only. Never install tools on a developer's PC.
set -euo pipefail
if [[ "${GITHUB_ACTIONS:-}" != true || "${RUNNER_OS:-}" != Linux ]] ||
    [[ "$(lsb_release -is)" != Ubuntu || "$(lsb_release -cs)" != noble || "$(uname -m)" != x86_64 ]]; then
    echo 'This installer is only for Ubuntu 24.04 x64 GitHub Actions runners.' >&2
    exit 2
fi
(( $# > 0 )) || { echo 'Specify mpv, ffmpeg, lua5.4, libmpv-dev, libxkbcommon-dev and/or libwebkit2gtk-4.1-dev.' >&2; exit 2; }
for package; do
    # The -dev packages let the desktop app (embedded libmpv, xkbcommon keyboard, the WebKitGTK
    # "Sign in with Google" window) build and link.
    case "$package" in mpv|ffmpeg|lua5.4|libmpv-dev|libxkbcommon-dev|libwebkit2gtk-4.1-dev) ;; *) echo 'Unsupported CI media package.' >&2; exit 2;; esac
done
apt_options=(
    -o Acquire::Retries=1
    -o Acquire::http::Timeout=15
    -o Acquire::https::Timeout=15
    -o APT::Update::Error-Mode=any
)
install_media() {
    # A per-request timeout alone can still stall across a hundred packages.
    # Bound each whole operation, then allow one independent mirror attempt.
    timeout --kill-after=10s 60s sudo apt-get "${apt_options[@]}" update &&
        timeout --kill-after=10s 180s sudo apt-get "${apt_options[@]}" install -y --no-install-recommends "$@"
}

install_media_from() {
    # One mirror per attempt: a single unreachable source must not sink the
    # whole run, so every mirror gets its own sources list and update+install.
    local list="$1"
    shift
    if [[ "$list" == default ]]; then
        install_media "$@"
        return
    fi
    local sources
    sources="$(mktemp "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/vidcue-ubuntu.XXXXXX.list")"
    cat > "$sources" <<EOF
deb $list noble main restricted universe multiverse
deb $list noble-updates main restricted universe multiverse
deb $list noble-security main restricted universe multiverse
EOF
    # Apt drops download privileges to _apt, which must be able to read this file.
    chmod 644 "$sources"
    local status=0
    timeout --kill-after=10s 60s sudo apt-get "${apt_options[@]}" \
        -o "Dir::Etc::sourcelist=$sources" -o Dir::Etc::sourceparts=- update ||
        status=$?
    if ((status == 0)); then
        timeout --kill-after=10s 180s sudo apt-get "${apt_options[@]}" \
            -o "Dir::Etc::sourcelist=$sources" -o Dir::Etc::sourceparts=- \
            install -y --no-install-recommends "$@" || status=$?
    fi
    rm -f -- "$sources"
    return "$status"
}

# The runner's Azure mirror first, then Canonical's archive, then the kernel.org
# mirror CDN: one DNS or network outage must not freeze CI media setup.
for mirror in default https://archive.ubuntu.com/ubuntu https://mirrors.edge.kernel.org/ubuntu; do
    if install_media_from "$mirror" "$@"; then
        exit 0
    fi
    echo "Mirror $mirror failed; trying the next one." >&2
done
echo 'CI media setup failed on every mirror. No playback test result is available.' >&2
exit 1
