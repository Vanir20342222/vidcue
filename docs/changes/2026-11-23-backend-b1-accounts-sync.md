# Self-hosted backend: accounts, pairing and sync foundation (B1)

- **Date:** 2026-11-23
- **PR:** none yet
- **Milestone:** B1 (docs/backend/PLAN.md)
- **Platforms:** backend (new `apps/server`), docs
- **Status:** live verified (server only; no app client yet)

## What it does
Adds `vidcue-server`, a self-hosted Rust backend with PostgreSQL. An admin creates an
invite link; the invitee picks a username and password in the browser and must
confirm an authenticator (TOTP) code. Devices sign in with password + code, or join an
existing account by a single-use QR pairing code. Devices push offline changes for
resume positions and history; the later edit wins, retries are idempotent, and other
devices get a WebSocket notification and pull the change. Deployed to the owner's self-hosted
Proxmox container behind the HTTPS reverse proxy.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Unit tests | `cargo test -p vidcue-server` (TOTP RFC 6238 vectors, replay, throttle, validation, merge order, escaping) | 13 passed | fixture/unit |
| API end to end | `VIDCUE_TEST_DATABASE_URL=… cargo test -p vidcue-server` against local PostgreSQL 17 | 14 passed | fixture/unit |
| Lint | `cargo clippy -p vidcue-server --all-targets -- -D warnings`, `cargo fmt -p vidcue-server -- --check` | clean | — |
| Live smoke | Throwaway account over the internet path via NPM: signup + TOTP, QR pairing, push on device A, WebSocket `changed` on device B, pull, socket alive after 35 s; account deleted afterwards | passed | live |

## Not verified and limits
- No Vidcue app client exists yet (B3); nothing in Android uses the server.
- Only `positions` and `history` sync. Library, settings, filters and E2E cookie sync are B2.
- No account recovery or TOTP reset flow; an admin must reset via SQL for now.
- Login throttling is per username and in memory (resets on restart).
- Web admin page, handoff, Jam and personal services are later milestones.
- Full `cargo test --workspace` was not run in this branch; only the new crate.

## For the plan and parity tables
- B1 Accounts and sync foundation: implemented and live verified on the deployed server (server only, 2026-11-23).
