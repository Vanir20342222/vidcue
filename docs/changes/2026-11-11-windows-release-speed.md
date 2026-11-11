# Faster Windows desktop release build (M9)

- **Date:** 2026-11-11
- **PR:** #144
- **Milestone:** M9
- **Platforms:** CI-release
- **Status:** implemented (static checks only)

## What it does
Every `desktop-*` tag compiled the whole dependency tree from scratch: about 16.5 of the 19
minutes of run 42989380374. Each tag saved its cache under its own ref, which the next tag cannot
read. Pushes to main that touch the desktop build now compile it without publishing and save a
shared cache (`windows-gnu-release`) that tag runs restore. Those builds also catch Windows-only
breakage before a tag. The workflow now uses the image's preinstalled Inno Setup (choco is only a
fallback) and MSYS2 (`release: false`). It is pinned to `windows-2025`, has a timeout, and cancels
superseded main builds but never a tag. Manual and main runs build the installer with a
Cargo.toml-derived version instead of the branch name, which Inno Setup rejected as a file version.
Manual runs upload the outputs as a 7-day artifact. The mpv-dev package is pinned to mpv-winbuild
`2026-11-10-bcde6b93cc` and checked by SHA-256 instead of taking whatever release is newest.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Workflow syntax and expressions | actionlint 1.7.7 | pass | static |

## Not verified and limits
Not run on a Windows runner from this branch. The speed-up applies from the first tag after a main
push has saved the cache.

## For the plan and parity tables
- Windows desktop release: tags restore a Rust cache warmed by main; main pushes compile-check Windows.
