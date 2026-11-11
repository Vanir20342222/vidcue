# CI media setup survives a single failing Ubuntu mirror (M9)

- **Date:** 2026-11-11
- **PR:** #<number> <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** CI only (GitHub-hosted Ubuntu runners)
- **Status:** offline regression suite passes; live CI proof on this PR

## What it does
`tools/ci-install-media.sh` now tries one mirror per attempt — the runner's Azure mirror,
Canonical's `archive.ubuntu.com`, then the kernel.org mirror CDN — each with its own sources
list and update+install. Previously the whole fallback used one combined sources list with
`APT::Update::Error-Mode=any`, so a single unreachable source (today: `security.ubuntu.com`
stopped resolving entirely, taking `archive.ubuntu.com` with it for an hour) failed the whole
run and froze every PR's `media` job.

## Why
On 2026-11-11 the Ubuntu mirror DNS outage (no A/AAAA records for
`security.ubuntu.com`/`archive.ubuntu.com` from multiple vantage points) failed the `media`
job on ten consecutive CI attempts for PR #148 between 15:09 and 16:00 UTC. The script's
all-or-nothing fallback could not use a mirror that was actually reachable
(`mirrors.edge.kernel.org` served noble/noble-updates/noble-security with HTTP 200 throughout).

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Offline regression | `bash tools/test-ci-install-media.sh` | 8 cases pass (incl. new `canonical_failure`: default + Canonical fail, kernel.org recovers; per-source isolation; deadlines) | unit |
| Syntax | `bash -n tools/ci-install-media.sh` | clean | static |
| Live CI | this PR's `media` job | green with the new installer | live CI |

## Not verified and limits
- The Ubuntu mirror outage itself is upstream (Canonical); this change only removes the CI's
  dependence on any one mirror.
- The kernel.org mirror is a third-party CDN (Fastly); its content parity for noble suites was
  checked live (InRelease fetches, HTTP 200), not by package-level diffing.

## For the plan and parity tables
- CI resilience fix; no user-facing behavior. No plan or parity change.
