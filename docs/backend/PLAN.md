# Self-hosted backend plan

A personal backend for Vidcue: account sync across devices, Jam (shared playback with
friends) and a few personal services. For the owner and invited friends only, never a
public service. The server code is `apps/server` (`vidcue-server`).

## Decisions (owner, 2026-11-23)

| Area | Decision |
| --- | --- |
| Hosting | Self-hosted Proxmox LXC container with local PostgreSQL, published over HTTPS through the owner's Nginx Proxy Manager (wildcard certificate) |
| Stack | Rust (axum, WebSockets) and PostgreSQL. Shared models come from `vidcue-core`. `tokio-postgres` instead of `sqlx`: sqlx pins a `libsqlite3-sys` that conflicts with the workspace's `rusqlite` |
| Accounts | Invite link → username + password + mandatory authenticator (TOTP). Further devices of the same account join by QR pairing |
| Admin | Admin screen in the app plus a small web admin page |
| Sync scope | History and resume positions, library (saved items, playlists, subscriptions, collection watches, offline playlist definitions), settings (themes, EQ, playback/download preferences), handoff and remote control |
| Offline | Offline-first: devices keep working and merge on reconnect without losing edits |
| Privacy | The server may read synced data (needed for recommendations and stats). Provider sign-in sessions (cookies) sync only end-to-end encrypted; the key travels in the pairing QR and never reaches the server. Friends see activity only after opting in |
| Jam | Friends with accounts, remote watch party (~300–500 ms tolerance). The server coordinates (queue, play/pause/seek, clock); every device resolves and streams the media itself. Spotify-style control: anyone adds to the queue, the host decides who may skip/seek/pause. Queue voting |
| Social | Collaborative playlists, send to a friend |
| Personal | Synced filters, private recommendations (subscriptions, related items, music similarity via open data), stats / yearly review, playback health log |
| Not wanted | Media server / scheduled archiving, upload push notifications, server-side stream resolving, web/TV client |
| First milestone | Sync |

## Milestones

- **B1 Accounts and sync foundation** (implemented 2026-11-23): invites, signup with TOTP,
  login, QR pairing, device list/revocation, register sync with push/pull and WebSocket
  change notifications; `positions` and `history` collections; deployed to the owner's server.
- **B2 Library, settings, filters and recovery** (server implemented 2026-11-23):
  per-entry playlist registers with fractional ordering, saved items, subscriptions,
  collection watches, offline playlist definitions, portable settings, EQ presets,
  synced filters, E2E-encrypted provider sessions with a key relay for devices that
  signed in by password. Recovery codes, password and authenticator changes, admin
  reset links, throttling stored in PostgreSQL. Client encryption lands with B3.
- **B3 Android client**: sign-in and pairing screens, outbox/op log in the Rust engine
  (`vidcue-mobile`), background sync, conflict-free merge into local stores.
- **B4 Handoff, remote control and Jam**: presence over the existing WebSocket, transfer
  playback, control another device, Jam rooms with server clock sync, queue voting,
  host permissions; send to a friend; collaborative playlists.
- **B5 Personal services**: playback health log (sanitized categories only), stats and
  yearly review, private recommendations, web admin page.

## Sync model

Every value is a register `(collection, key)` per user. A device pushes operations
`{id, collection, key, value | null, clock}`; `null` is a tombstone. The later client
clock wins, the device id breaks ties, and clocks more than five minutes ahead are
clamped. Operation ids make retries idempotent. Pushes per user are serialised, so
`GET /api/sync/pull?since=<seq>` never skips a change. Collections validate their values;
data that only makes sense on one device (local file paths) is rejected. Finer merges,
such as playlists, use one register per entry.

## Security notes

Reviewed 2026-11-23; see docs/changes/2026-11-23-backend-security-hardening.md.

- Tokens (devices, invites, resets, pairing codes, WebSocket tickets) are random and
  stored only as SHA-256. Invite and reset tokens travel in URL fragments; device
  tokens never appear in URLs. Unused device tokens expire after 180 days.
- Passwords use Argon2id with at most 4 hashes at a time; unknown usernames cost the
  same as wrong passwords.
- Authenticator seeds are AES-256-GCM encrypted and recovery codes are HMACs, both
  with a server key kept outside the database (systemd credential).
- TOTP codes cannot be replayed. Throttles are recorded before checking (race-free),
  per client address and per user, and survive restarts.
- Pairing and admin invites need the password; password, authenticator, recovery-code
  changes and admin resets need password plus a current code. Revoking a device
  revokes the devices it paired.
- Synced records are rebuilt from known fields; device-local download settings
  (post-download hook, folders, endpoints) never sync.
- Only the reverse proxy reaches the service (app check plus nftables); Postgres listens
  only on its Unix socket with peer authentication.
