# Prebuilt Linux desktop binary on each desktop release (M9)

- **Date:** 2026-11-26
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** desktop, CI-release
- **Status:** implemented (first verified by the release run it ships with)

## What it does
The owner asked (2026-11-26) for a Linux build they can download and use directly. The desktop
release workflow (`windows-release.yml`) gains a `linux` job on Ubuntu 24.04. It builds
`vidcue-desktop`, checks that the binary starts (`--list-scenes`), and attaches
`vidcue-linux-x86_64.tar.gz` to every `desktop-*` release. The tarball holds the binary
(`vidcue`), the icon, a README and `install.sh` (`tools/linux/install-binary.sh`).

The installer puts the binary in `~/.local/lib/vidcue/` and links `~/.local/bin/vidcue` to it.
Because the running executable is then not the `install.sh` launcher, the in-app updater offers
the release page instead of rebuilding from source. An existing source-built launcher is kept
as `vidcue.source-build`. The installer lists any missing runtime libraries (libmpv,
WebKitGTK 4.1, libxkbcommon).

The workspace version goes to 0.2.0-alpha.20 for the release that carries the desktop redesign
(#185) and this binary.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Workflow parses | `python3 -c "yaml.safe_load(...)"` | jobs `windows`, `linux` | unit |
| Installer syntax | `bash -n tools/linux/install-binary.sh` | ok | unit |
| Binary start check used by the job | `vidcue-desktop --list-scenes` on a local release build (earlier the same day) | lists scenes | local |
| Lockfile after the version bump | `cargo metadata --locked` | ok | unit |

## Not verified and limits
- The `linux` job and the installer have not run on GitHub or on the owner's PC before this
  release. The release run is their first test.
- Runtime libraries are not bundled. The build needs glibc 2.39 or newer and the distribution's
  libmpv (soname 2), WebKitGTK 4.1 and libxkbcommon.

## For the plan and parity tables
- Desktop releases ship a prebuilt Linux x86_64 tarball with a user-level installer
  (2026-11-26).
