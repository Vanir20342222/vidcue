# Self-hosted backend: security hardening after review (B1/B2)

- **Date:** 2026-11-23
- **PR:** none yet
- **Milestone:** B1, B2 (docs/backend/PLAN.md); follows 2026-11-23-backend-b2-library-recovery.md
- **Platforms:** backend (`apps/server`), deployment (backend container, reverse proxy)
- **Status:** live verified (server only; no app client yet)

## What it does
Fixes the findings of the 2026-11-23 security review of `vidcue-server` and its deployment:

- **Login flooding (H1):** password hashing runs at most 4 at a time, and reset links are checked before hashing.
- **Throttle races (H2):** attempts are recorded before the check, so parallel requests cannot slip past the limit.
- **Synced download settings (H3):** the post-download hook, folders and service endpoints never sync, in settings or in collection watches.
- **Shared throttles and lockouts (M1, M2):** throttles are per client address and per user instead of global, so one person cannot lock out someone else.
- **Pool and storage exhaustion (M3, M4):** pushes are serialized per user before taking a connection, the pool and statements have timeouts, and each user has a quota. Hourly cleanup removes expired rows, there are caps on devices and open pairings, and device tokens expire after 180 days.
- **Tokens in URLs (M5):** invite and reset tokens move to URL fragments, and WebSockets use single-use tickets instead of device tokens in URLs.
- **Stolen device tokens (M6):** pairing and admin invites need the password. Changing the password, the authenticator, the recovery codes, or using an admin reset needs the password plus a current code. Revoking a device also revokes the devices it paired.
- **WebSocket and stored values (L1–L3):** a socket cap per device and a message size limit. NUL characters are rejected, and every collection stores only known fields.
- **Secrets at rest (L5):** authenticator secrets are encrypted and recovery codes are keyed hashes, using a server key held outside the database.
- **Direct access (L6):** only Nginx Proxy Manager can reach the service, enforced in the app and with nftables.
- **systemd unit (L7):** a stricter sandbox plus memory and task limits.
- **Page script (L8):** the CSP pins the inline script by hash.

Deployment:
- The backend container has all OS updates applied, unattended upgrades enabled and key-only SSH.
- SSH on the reverse proxy host now accepts keys only.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Server tests | `VIDCUE_TEST_DATABASE_URL=… cargo test -p vidcue-server`: incl. 20 parallel bad logins (exactly 5 reach the check), per-address separation, throttle across restart, proxy-only refusal, cascade revocation, sealed TOTP seed, keyed recovery hashes, download hook refusal, maintenance | 26 passed | fixture/unit |
| Lint | `cargo clippy -p vidcue-server --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `cargo check --workspace --all-targets --locked` | clean | — |
| Deploy | Pre-upgrade `pg_dump`; migration 3 applied; existing authenticator seed encrypted at start; `systemd-analyze security` 4.1 → 1.2 | done | live |
| Exposure | Direct LAN request to the service port times out; public HTTPS works; SSH answers `Permission denied (publickey)` | as intended | live |
| Live smoke | Throwaway account through NPM: CSP hash header, signup via fragment link, pairing needs password, hook refused, WebSocket token-in-URL refused and single-use ticket works, cascade revocation closes the socket, login throttle (recorded under the real client address, not the proxy's); account deleted | passed | live |

## Not verified and limits
- **Not changed:** other services on the same host are outside this backend.
- **Not changed:** the router's port forwards.
- **L4 (timing) only partly fixed:** a correct password with a recovery-style code still costs one more DB update than a wrong password. Per-address and per-user throttles bound what can be learned from it.
- **No off-host backups yet:** dumps no longer contain readable authenticator secrets, but they are not encrypted and stay on the host. The server key is not backed up off-host either.
- **No app client yet (B3).**

## For the plan and parity tables
- Backend security review 2026-11-23: all high and medium findings fixed and live verified on the deployed server.
