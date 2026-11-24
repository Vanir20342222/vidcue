# Self-hosted backend: library sync, encrypted sessions and account recovery (B2)

- **Date:** 2026-11-23
- **PR:** none yet
- **Milestone:** B2 (docs/backend/PLAN.md); follows 2026-11-23-backend-b1-accounts-sync.md
- **Platforms:** backend (`apps/server`), shared engine (`vidcue-core`: `equalizer::valid_name` is now public)
- **Status:** live verified (server only; no app client yet)

## What it does
The server now syncs the library and settings too: saved items, playlists (one register
per entry with fractional ordering, so edits on two offline devices merge), offline
playlist definitions, collection watches, subscriptions, portable settings checked
by `Config::validate`, EQ presets and content filters. Provider sessions sync only as
ciphertext with a fixed shape the server checks but cannot decrypt; a device that signed in
without QR pairing obtains the key through a relay that carries only a public key and
a sealed key. Accounts can recover: ten one-time recovery codes at signup, password
and authenticator changes, and admin reset links (`vidcue-server reset-link USER` or
the admin API) that sign out every device. Failed-attempt throttling moved from
memory to PostgreSQL.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Unit tests | `cargo test -p vidcue-server` (collection validation incl. settings, EQ, secrets, playlists, offline playlists, filters; recovery code format) | passed | fixture/unit |
| API end to end | `VIDCUE_TEST_DATABASE_URL=… cargo test -p vidcue-server`: recovery login and reuse, throttle after a simulated restart, password change signing out others, authenticator replacement, key relay, B2 push/pull, admin reset (devices revoked, other users untouched, synced data kept) | 21 passed | fixture/unit |
| Workspace | `cargo test --workspace --locked` | 475 passed, 0 failed (13 test binaries) | fixture/unit |
| Live | Deployed after a pre-upgrade `pg_dump`; migration 0002 applied; throwaway account over the public HTTPS URL: 10 recovery codes at signup, recovery-code login and reuse refused, B2 push/pull with a device-local setting refused, key relay with WebSocket `key_request` event, password change signing out the other device; account deleted | passed | live |

## Not verified and limits
- No app client yet (B3): nothing encrypts sessions or computes fractional positions on a device today.
- The key relay's protection against a malicious server depends on the client showing and the user comparing the key fingerprint; that UI is B3.
- No self-service password reset by email (there is no email); recovery codes or an admin link are the paths.
- Collection watches are validated with the full core type, so clients must send every field.

## For the plan and parity tables
- B2 Library, settings, filters and recovery: server implemented and live verified on the deployed server (2026-11-23); clients pending (B3).
