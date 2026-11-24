# vidcue-server

Self-hosted Vidcue backend. Plan and decisions: [docs/backend/PLAN.md](../../docs/backend/PLAN.md).

## Run

```sh
DATABASE_URL="host=/run/postgresql user=vidcue dbname=vidcue" \
VIDCUE_PUBLIC_URL=https://vidcue.example.org \
vidcue-server serve --bind 127.0.0.1:8080
vidcue-server invite --admin      # prints a one-time signup link (default 72 h)
vidcue-server reset-link alice    # 48 h link: new password + authenticator, signs out all devices
```

Migrations run on every start. `deploy/install.sh` (`PUBLIC_URL=… PROXY_IP=… sh install.sh ./vidcue-server`)
installs the binary, a sandboxed systemd unit with memory/task limits, a
peer-authenticated Postgres role, an nftables rule that lets only the reverse proxy
reach the service port, and a nightly `pg_dump` timer (`/var/backups/vidcue`, 14 kept).

**Server key.** `serve` needs a 32-byte key (`vidcue-server gen-key`; the installer
creates `/etc/vidcue/secret.key`, root-only, passed in as a systemd credential). It
encrypts authenticator seeds and keys the recovery-code hashes, so a database dump
alone reveals neither. Back it up *separately* from the database: without it,
existing authenticators stop working and every user needs a reset link.

| Variable | Meaning |
| --- | --- |
| `VIDCUE_TRUSTED_PROXIES` | Proxy addresses whose `X-Real-IP` is trusted (throttles are per client address) |
| `VIDCUE_PROXIES_ONLY` | `true`: refuse direct connections from anything but those proxies and loopback |
| `VIDCUE_SECRET_KEY_FILE` | Key file when not run under systemd |

## API (B1, B2)

All bodies are JSON. Authenticated calls send `Authorization: Bearer <device token>`.

| Method and path | Purpose |
| --- | --- |
| `GET /join#<invite>` | Browser signup page; the token stays in the URL fragment, which browsers never send, so it never reaches proxy logs |
| `POST /api/signup` `{invite, username, password}` | Starts signup, returns TOTP secret, `otpauth://` URI and QR SVG |
| `POST /api/signup/confirm` `{invite, signup_id, code, device_name?}` | Activates the account; with `device_name` also returns a device token |
| `POST /api/login` `{username, password, code, device_name?}` | Returns a device token |
| `GET /api/me`, `GET /api/devices`, `DELETE /api/devices/{id}` | Account and devices; revoking a device also revokes every device it paired. At most 20 devices; tokens unused for 180 days expire |
| `POST /api/pairing` `{password}` | Single-use 10-minute pairing code (at most 5 open), `vidcue://pair?server=…&code=…` URI and QR SVG |
| `POST /api/pairing/claim` `{code, device_name?}` | New device of the same account gets a token |
| `POST /api/sync/push` `{ops: [{id, collection, key, value?, clock}]}` | Per-op `applied` / `superseded` / `duplicate` / `rejected` |
| `GET /api/sync/pull?since=&limit=` | Changes after `since`, `next`, `more` |
| `POST /api/ws/ticket` | 30-second single-use ticket for `GET /api/ws?ticket=…` (device tokens are never accepted in URLs) |
| `GET /api/ws` | Bearer header or `?ticket=`; at most 3 sockets per device, server-to-client only. Events: `hello {seq}`, `changed {seq, device_id}`, `device_added`, `device_revoked`, `devices_revoked {keep?}`, `key_request`, `key_granted` |
| `POST /api/admin/invites` `{password, admin?, hours?}`, `GET /api/admin/users` | Admin only |
| `POST /api/admin/users/{id}/reset` `{password, code}` | Admin only: reset link for a user |
| `GET /reset#<token>`, `POST /api/reset` `{token, password}`, `POST /api/reset/confirm` `{token, code}` | Reset flow; confirm signs out every device and returns new recovery codes |
| `GET /api/account/recovery-codes`, `POST /api/account/recovery-codes` `{password, code}` | Remaining count; regenerate (shown once) |
| `POST /api/account/password` `{current_password, code, new_password, sign_out_others?}` | Change password |
| `POST /api/account/totp` `{password, code}`, `POST /api/account/totp/confirm` `{code}` | Replace the authenticator |
| `POST /api/keys/requests` `{public_key}`, `GET /api/keys/requests`, `GET /api/keys/requests/{id}`, `POST /api/keys/requests/{id}/grant` `{sealed_key}` | E2E key relay for devices that signed in without QR pairing |

`code` is always an authenticator code or a one-time recovery code (`XXXXX-XXXXX`).
Signup confirmation returns ten recovery codes.

**Throttling** (PostgreSQL `auth_failures`, survives restarts; each attempt is recorded
before the slow check runs, so parallel requests cannot slip past the limit):
logins 10 per address / 15 min, 5 per user and address / 15 min, 100 per user / hour;
5 wrong second factors per user / hour after a correct password; invite, pairing and
reset codes 30 per address / 10 min; re-authentication 5 per user / 15 min. At most 4
password hashes run at once (503 when busy). Per user: 100 000 records, 64 MiB.

### Sync collections

| Collection | Key | Value |
| --- | --- | --- |
| `positions` | `PlaybackHistory::key` | `vidcue_core::playback_history::Position`, online media only |
| `history` | event id | object with an online `url` |
| `saved` | `PlaybackHistory::key` | `Media` |
| `playlists` | playlist UUID | `{title, ...}` |
| `playlist_entries` | `<playlist UUID>/<entry UUID>` | `{position, media}`; `position` is a fractional index string (`[0-9A-Za-z]{1,64}`), so concurrent inserts and moves never conflict |
| `offline_playlists` | playlist id | `vidcue_core::offline::OfflinePlaylist` (metadata; files stay on the device) |
| `collection_watches` | watch id | `vidcue_core::watch::CollectionWatch`; its download options must leave hook, folders, template and endpoints at their defaults |
| `subscriptions` | channel id | `{channel_id, title, ...}` |
| `settings` | `Config` field name | field value; only portable fields (see `PORTABLE_SETTINGS`), validated through `Config::validate`. `download` accepts only `PORTABLE_DOWNLOAD_FIELDS`: the post-download hook, folders, filename template and service endpoints never sync |
| `eq_presets` | preset name | `{gains: [10 × dB]}` |
| `filters` | UUID | `{kind: channel\|keyword\|shorts\|watched\|live, value?}` |
| `secrets` | provider (`youtube`, ...) | `{v: 1, alg: "xchacha20poly1305", nonce, ciphertext}`, base64; encrypted on the device with the account key, never readable by the server |

### End-to-end key

Provider sessions are encrypted with a 32-byte account key that only devices hold.
QR pairing carries it in the URI fragment that the app adds locally
(`vidcue://pair?server=…&code=…#k=<key>`); the server only sees the code. A device
that signed in with a password posts an X25519 public key to `/api/keys/requests`;
a trusted device shows a fingerprint of that key, the user compares it with the
fingerprint on the new device, and the trusted device posts the key sealed to it.

## API (B4)

Friends, activity sharing, send-to-a-friend, shared playlists, presence, handoff,
remote control and Jam. The wire format is `docs/backend/PROTOCOL.md` §1–§5. Objects
the caller may not see answer 404, so ids cannot be probed. Media is always stored in
the slim online form; local files are refused.

| Method and path | Purpose |
| --- | --- |
| `GET /api/friends` | `friends` (accepted), `incoming` and `outgoing` requests, by username |
| `POST /api/friends/requests` `{username}` | `requested`, or `accepted` when they had already asked; unknown, inactive or own name → 404 |
| `POST /api/friends/{user_id}/accept`, `DELETE /api/friends/{user_id}` | Accept; decline, cancel or remove. Removing a friend also drops each from the other's shared playlists and Jam room and withdraws Jam invites between them |
| `GET`/`PUT /api/social/settings` `{share_activity: off\|now_playing\|history}` | Opt-in activity sharing (default `off`) |
| `GET /api/friends/activity` | Opted-in friends: `now_playing` from live presence, plus the last 20 history events of 7 days for `history` |
| `POST /api/inbox` `{to, media, note?}`, `GET /api/inbox?unread_only=`, `POST /api/inbox/{id}/read`, `DELETE /api/inbox/{id}` | Send to a friend; 200 items per inbox (oldest read items make room), notes ≤ 200 printable characters, kept 90 days |
| `POST /api/shared-playlists` `{id, title}`, `GET /api/shared-playlists` | Create (idempotent per owner) and list shared playlists |
| `PATCH`/`DELETE /api/shared-playlists/{id}` | Owner: rename or delete |
| `POST /api/shared-playlists/{id}/members` `{user_id}`, `DELETE …/members/{user_id}` | Owner adds friends (≤ 20 members) or removes anyone; members leave |
| `POST /api/shared-playlists/{id}/push` `{ops: [{id, entry_id, value?, clock}]}` | Per-entry registers like `playlist_entries`: later clock wins, op ids idempotent, ≤ 2000 entries |
| `GET /api/shared-playlists/{id}/pull?since=&limit=` | Entry changes, `playlist`, `next`, `more`; a complete pull also moves `next` past title/member changes |
| `POST /api/jam` `{title?, guests_can_control?}`, `GET /api/jam` | Start a room (ends your hosted room, leaves your current one); current room and open invites |
| `GET`/`PATCH`/`DELETE /api/jam/{id}` | Room for members; host edits settings or ends it |
| `POST /api/jam/{id}/invite` `{user_ids}`, `…/join`, `…/decline`, `…/leave`, `DELETE …/members/{user_id}` | Members invite their own friends; join needs an invite (≤ 10 members); host leaving ends the room; host removes members |
| `POST /api/jam/{id}/queue` `{media}`, `DELETE …/queue/{item_id}`, `POST …/queue/{item_id}/vote` `{up}` | Anyone queues (≤ 200; the first item starts at once), host or adder removes, members vote (most votes first, then oldest) |
| `POST /api/jam/{id}/playback` `{action: play\|pause\|seek\|next\|play_item\|ended\|rate, …}` | Host, or guests when `guests_can_control`; `ended` from any member advances once |

Rooms end when the host ends or leaves them, after 30 minutes without a room action or
`jam_ping`, or 12 hours after creation; ended rooms are deleted a day later.

**WebSocket (client to server).** `hello` lists `features: ["realtime","social","jam"]`
and `server_ms`. Text frames of at most 32 KiB, one JSON object with `type` and an
optional `rid` (answered by exactly one `ack` or `error`): `ping {t0}` → `pong
{t0, server_ms}` (Jam clock), `presence {state}`, `presence_query`, `handoff` and
`remote` (to another connected device of the same user only), `jam_ping {room_id}`.
Presence lives in memory. Limits per socket: one `presence` per 2 s, 20 other messages
per 10 s; three malformed or unknown messages within a minute close the socket (1008),
binary frames close it (1003). Server events: `presence`, `presence_gone`, `handoff`,
`remote` (only on the target device), `friend_request`, `friend_added`,
`friend_removed`, `friend_activity` (at most one per sharing user per 10 s; changes
inside the window are coalesced into one event at its end), `inbox_item`,
`shared_playlist_changed`, `shared_playlist_removed`, `jam_state`, `jam_changed`,
`jam_invite`, `jam_ended`. A missed-event overflow sends `changed {seq, resync: true}`.

**Throttling** (same table as B1, never forgiven): friend requests 20 / user / hour,
inbox sends 60 / user / hour, Jam queue additions 30 / user / 10 min, Jam playback
actions 60 / user / minute, Jam rooms 10 / user / hour.

## Tests

`cargo test -p vidcue-server` runs offline unit tests. The end-to-end API test needs a
disposable database and skips without one:

```sh
VIDCUE_TEST_DATABASE_URL=postgres://user:pass@127.0.0.1/vidcue_test cargo test -p vidcue-server
```

It drops and recreates the `public` schema of that database.

## API (B5)

Contract: [docs/backend/PROTOCOL.md](../../docs/backend/PROTOCOL.md) §1 and §6.

| Method and path | Purpose |
| --- | --- |
| `GET /api/features` | No auth. `{features: [health, stats, recs, admin_web], recs_available}` |
| `POST /api/health/reports` `{reports: [{kind, category, client_version, platform, at, duration_ms?}]}` | Sanitized playback outcomes only: fixed `kind`/`category`/`platform` lists, no other fields. At most 100 per request, each checked on its own (`{accepted, rejected}`); 2000 accepted per user per 24 h; 30 uploads per device per hour; kept 90 days |
| `GET /api/health/summary?days=7` | Own reports: totals and ok rates per kind, top problem categories, start time p50/p90, per UTC day (1 to 90 days) |
| `GET /api/stats?from=&to=&tz_offset_min=` | Period stats from the user's synced `history` (at most 366 days; plays under 5 s ignored; days and hours in the given offset) |
| `GET /api/stats/wrapped?year=&tz_offset_min=` | Yearly review, cached per user and year (refreshed after 6 h, final once computed a week after the year ended). Statistics computations: 60 per user per 10 min |
| `GET`/`PUT /api/recs/settings` `{enabled?, music?}` | Recommendations are off until the user turns them on; turning them off deletes the feed |
| `GET /api/recs` | `{enabled, built_at, building, items: [{media, reason, source}]}`; starts a background build when the feed is older than 6 h |
| `POST /api/recs/refresh` | 202; one build per user per 30 min (429) |
| `GET /api/admin/users/{id}/devices` | Admin: a user's devices (never tokens) |
| `POST /api/admin/users/{id}/devices/revoke` `{password, device_id\|null}` | Admin: revoke one device (and the devices it paired) or all |
| `POST /api/admin/users/{id}/disable`, `.../enable` `{password}` | Admin: a disabled user cannot sign in or use any token; their devices, pairings, resets, key requests and admin sessions are removed. Not yourself, never the last enabled admin |
| `GET /api/admin/health?days=7` | Admin: health over all users plus `users_reporting` and `by_version` |
| `GET /admin` | Web admin page (below) |

**Recommendations** come from the user's followed channels (newest uploads), items
related to recently finished plays, and songs by artists similar to the user's top
artists (MusicBrainz + ListenBrainz Labs). The server uses public YouTube pages without
any session and never sees provider cookies. Upstream answers are cached and shared
(`recs_cache`, at most 50 000 rows); each upstream gets at most one request per second,
10 s timeout, 2 MiB answers, and all users together at most `VIDCUE_RECS_FETCH_PER_HOUR`
requests. A build that runs out of budget stores what it has.

| Variable | Meaning |
| --- | --- |
| `VIDCUE_RECS` | `on` (default) or `off`: off answers 404 on `/api/recs*` |
| `VIDCUE_RECS_MUSIC` | `on` (default) or `off`: MusicBrainz/ListenBrainz lookups |
| `VIDCUE_RECS_FETCH_PER_HOUR` | Upstream requests per hour over all users (default 120) |

**Web admin page** (`/admin`): server-rendered forms, no JavaScript. Sign in with
username, password and code (same throttles as the app, counted separately). Only
enabled admins get in; anyone else sees the same error as a wrong password. The
session cookie is `HttpOnly; Secure; SameSite=Strict; Path=/admin`; the session ends
after 30 idle minutes or 8 hours, at most 3 per admin, and the server stores only its
SHA-256. Every action form carries a per-session CSRF token and the password (reset
links also a code); posts with a foreign `Origin` or a `Sec-Fetch-Site` other than
`same-origin` are refused. The CSP allows only the page's own stylesheet (by hash) and
form posts to itself. The page shows users, a one-time invite or reset link with its QR
code, and the 7-day health table; it never shows device tokens, authenticator secrets
or recovery codes. Behind a reverse proxy, `/admin` needs `VIDCUE_PUBLIC_URL` to be the
exact public origin.
