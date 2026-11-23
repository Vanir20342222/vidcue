# Backend protocol contract: B3, B4, B5

Status: binding contract for one parallel implementation pass (2026-11-23). Four agents
implement it without talking to each other: **server-b4**, **server-b5**, **client-core**
and **android-ui**. When the contract is ambiguous, pick the reading that is safest and
smallest, write the choice in your change note, and do not change anything another agent
owns. Read [PLAN.md](PLAN.md), [apps/server/README.md](../../apps/server/README.md) and the
three `docs/changes/2026-11-23-backend-*.md` notes first.

Conventions used everywhere below:

- All times are Unix **milliseconds** (`i64`) unless a field name ends in `_s` or the
  field is a legacy core field (`Position.seconds`, history `seconds`/`duration`).
- All ids are UUID strings (v4) unless stated. `user_id`, `device_id` are the B1 ids.
- `Media` is `vidcue_core::Media` serialized as JSON. "Online media" means
  `vidcue_core::http_url(media.url)` succeeds. "Slim media" means the server's
  `sync::slimmed` form (`description` = "", `chapters` = [], no `offline_id`).
  Every server endpoint that stores media stores slim online media and rejects local
  media with 400.
- `media_key` is always `vidcue_core::playback_history::PlaybackHistory::key(&media)`.
- HTTP errors keep the B1 shape: status code plus `{"error": "<human message>"}`.
  Messages must never echo tokens, cookies, URLs from the request or SQL errors.
- Authenticated endpoints take `Authorization: Bearer vcd_…` (the B1 `Device`
  extractor). "Admin" means `device.is_admin`, else 403 `{"error":"Admins only"}`.
- "Friend" always means an **accepted** friendship (both sides agreed).
- Every new limit below is a named `const` in the module that enforces it.
- Throttles that count actions use `AppState::attempt` (record-before-check, never
  forgiven for action counters) with **new `Rule` constants defined in the owning
  module**, not in `throttle.rs`. Keys are `"<rule-name>:<user uuid>"`.

---

## 0. Ownership and shared files

### 0.1 Who owns what

| Area | Owner | Files (create unless noted) |
| --- | --- | --- |
| B4 server | server-b4 | `apps/server/migrations/0004_social_realtime.sql`, `apps/server/src/{social,presence,jam}.rs`, `apps/server/src/tests/social_realtime.rs`, `docs/changes/2026-11-23-backend-b4-social-realtime.md` |
| B5 server | server-b5 | `apps/server/migrations/0005_personal.sql`, `apps/server/src/{health,stats,recs,admin_web}.rs`, `apps/server/src/tests/personal.rs`, `apps/server/tests/fixtures/recs/*`, `docs/changes/2026-11-23-backend-b5-personal.md` |
| B3 client library + engine | client-core | `crates/vidcue-sync/**` (new crate), `crates/vidcue-mobile/src/sync_bridge.rs`, `crates/vidcue-mobile/src/sync_stores.rs`, `crates/vidcue-mobile/src/social_ops.rs` (new), `docs/changes/2026-11-23-backend-b3-sync-client.md` |
| B3 Android UI | android-ui | new Kotlin/Java files listed in §9, `apps/android/app/src/test/java/dev/ytui/android/{Sync*,Qr*,Jam*,Social*}Test.kt`, `docs/changes/2026-11-23-backend-b3-android-ui.md` |

Nobody edits: `RUST_REWRITE_PLAN.md`, `docs/VALIDATION.md`, `docs/android/{PLAN,PARITY,VALIDATION}.md`,
`docs/backend/PLAN.md`, this file, workflows, versions, `throttle.rs`, `auth.rs`,
`secrets.rs`, `keys.rs`, `web.rs`, `vidcue-core`, `vidcue-providers`.

### 0.2 Exact edits allowed in shared files

Each agent edits **only** the listed region of a shared file, at the listed anchor, so
the branches merge with at most the trivial conflicts named in §0.3.

**Root `Cargo.toml`** — client-core only: add `"crates/vidcue-sync"` to `members`
(insert after `"crates/vidcue-mobile"`). Nobody adds workspace dependencies; declare
versions in the crate's own `Cargo.toml`.

**`apps/server/Cargo.toml`**
- server-b4: append to `[dev-dependencies]` only:
  `tokio-tungstenite = { version = "0.29", default-features = false, features = ["connect"] }`
  and `futures-util = "0.3"` (plain `ws://127.0.0.1` in tests, no TLS).
- server-b5: append to `[dependencies]` only (after `tracing-subscriber`):
  `vidcue-providers = { path = "../../crates/vidcue-providers" }`,
  `reqwest = { workspace = true, features = ["rustls-tls"] }`, `async-trait = "0.1"`.

**`apps/server/src/main.rs`** (module list)
- server-b4: insert directly after `mod keys;`: `mod jam;` `mod presence;` `mod social;`
- server-b5: insert directly after `mod auth;`: `mod admin_web;` `mod health;` `mod recs;` `mod stats;`

**`apps/server/src/db.rs`** — append one tuple to `MIGRATIONS` after version 3:
server-b4 `(4, "social_realtime", include_str!("../migrations/0004_social_realtime.sql"))`,
server-b5 `(5, "personal", include_str!("../migrations/0005_personal.sql"))`.
The two migrations are independent (no cross references) and may apply in either order.

**`apps/server/src/api.rs`**
- server-b4 (only agent touching these): `Inner` gains `pub realtime: crate::presence::Realtime`
  (initialised with `Default::default()` in `AppState::new`); `Hub` changes in §3.4
  (channel capacity 256, `to_device` filtering, `SocketSlot`/online lookup made
  `pub(crate)`); `MAX_WS_MESSAGE` becomes `32 * 1024`; `ws_session` incoming arm and
  `Lagged` arm per §3.4; router: insert `.merge(crate::social::routes())`,
  `.merge(crate::jam::routes())` directly after `.merge(crate::keys::routes())`;
  `maintenance`: insert `crate::social::maintenance(db, now).await?;` and
  `crate::jam::maintenance(db, now).await?;` between the `batch_execute(...).await?;`
  statement and the final `Ok(())`.
- server-b5 (only agent touching these): router: insert
  `.merge(crate::health::routes())`, `.merge(crate::stats::routes())`,
  `.merge(crate::recs::routes())`, `.merge(crate::admin_web::routes())` directly after
  the `.route("/healthz", …)` line; `maintenance`: insert
  `crate::health::maintenance(db, now).await?;` and `crate::recs::maintenance(db, now).await?;`
  and `crate::admin_web::maintenance(db, now).await?;` as the first statements of the
  function body (before `let day = …`); the login query predicate
  `WHERE username = $1 AND active` becomes `WHERE username = $1 AND active AND disabled_at IS NULL`;
  `DEVICE_LOOKUP` `AND u.active` becomes `AND u.active AND u.disabled_at IS NULL`;
  `admin_users` adds `u.disabled_at` to its SELECT/GROUP BY and `"disabled": bool` to each user.
- server-b4 may change `fn online_media` and `fn slimmed` in `sync.rs` to `pub(crate) fn`
  (signature visibility only). No other `sync.rs` edit by anyone.

**`apps/server/src/account.rs`** — server-b5 only: `user_id()` query adds
`AND disabled_at IS NULL`; `admin_reset` refuses a disabled user with 409
`{"error":"Enable the user first"}`.

**`apps/server/src/tests.rs`** — test modules are children of `tests`, so they can call
its private helpers (`fresh_db`, `client`, `register`, `code_for`, `clear_throttles`,
`Client::{post,get,call}`) and share `DB_LOCK`. server-b4 inserts `mod social_realtime;`
directly after the last `use` line at the top; server-b5 appends `mod personal;` as the
last line of the file. Files live in `apps/server/src/tests/`.

**`apps/server/README.md`** — server-b4 inserts a `## API (B4)` section directly before
`## Tests`; server-b5 appends `## API (B5)` at the end of the file.

**`crates/vidcue-mobile`** — client-core only (android-ui never edits Rust):
- `Cargo.toml`: add `vidcue-sync = { path = "../vidcue-sync" }` and
  `qrcode = { version = "0.14", default-features = false }`.
- `lib.rs`: `mod sync_bridge; mod sync_stores; mod social_ops;` after `mod resolution;`;
  `MobileState` new fields (§8.3, all `#[serde(default)]`); `Engine` gains
  `sync: sync_bridge::SyncHandle`; one dispatch arm directly before
  `_ => anyhow::bail!("Unknown mobile operation")`:
  `op if sync_bridge::handles(op) => sync_bridge::dispatch(self, op, r).await,`;
  the change hooks of §8.4 inside existing arms; `save_settings` body extracted into
  `fn commit_config(&self, config: Config) -> Result<()>` so remote settings reuse it;
  the `browse`, `up_next` and `prepare_context` result filters of §8.5.
- `platform.rs`: add `pub(crate) fn ui_event(kind: &str, text: &str)` (static
  `MediaBridge.event(String,String)` via JNI; JNI ignores Java access, the method stays
  package-private) and a non-Android stub that forwards to a test collector.

**Android hotspots** — android-ui only, minimal call-site lines as listed in §9.3.

### 0.3 Merge order and expected trivial conflicts

Merge order: **server-b4 → server-b5 → client-core → android-ui**. Expected conflicts,
all "keep both sides": `db.rs` `MIGRATIONS` (keep versions 4 then 5), `Cargo.lock`
(take either side, then run `cargo check --workspace -j 2` without `--locked` to
regenerate). Anything else conflicting means someone left their region.

### 0.4 Server and client share no Rust crate in this pass

The wire format is this document. The server does **not** depend on `vidcue-sync`
(it would pull a WebSocket/TLS client stack into the server, and both sides are written
in parallel). `vidcue-sync::protocol` contains the client's serde types for every
message here; the server builds the same JSON in its modules. Both sides test against
the literal JSON examples in this document (copy them into tests). Unifying the types
behind a `protocol`-only feature of `vidcue-sync` is listed in Deferred.

---

## 1. Capability discovery

`GET /api/me` stays as is. The WebSocket `hello` event (B1) gains a `features` array
(server-b4 adds it):

```json
{"type":"hello","seq":812,"device_id":"…","server_ms":1763900000000,
 "features":["realtime","social","jam"]}
```

server-b5 cannot edit `ws_session`, so B5 features are discovered over HTTP:
`GET /api/features` (server-b5, no auth) →
`{"features":["health","stats","recs","admin_web"],"recs_available":true}`
(`recs_available` false when `VIDCUE_RECS=off`). A client sends WebSocket
client-to-server messages **only** when `hello.features` contains `"realtime"`; the
deployed B1/B2 server closes the socket on any client text frame. A 404 on a B4/B5
endpoint means "server too old": the client hides the feature, never retries in a loop.

---

## 2. B4 server: social (`social.rs`)

### 2.1 Tables (`0004_social_realtime.sql`)

```sql
-- Friendships: one row per unordered pair, user_a < user_b (UUID byte order).
CREATE TABLE friendships (
    user_a UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_b UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    requested_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    accepted_at BIGINT,               -- NULL while pending
    created_at BIGINT NOT NULL,
    PRIMARY KEY (user_a, user_b),
    CHECK (user_a < user_b)
);
CREATE INDEX friendships_b ON friendships(user_b);

CREATE TABLE social_settings (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    share_activity TEXT NOT NULL DEFAULT 'off'
        CHECK (share_activity IN ('off','now_playing','history')),
    updated_at BIGINT NOT NULL
);

CREATE TABLE inbox (
    id UUID PRIMARY KEY,
    to_user UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    from_user UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    media JSONB NOT NULL,             -- slim online Media
    note TEXT NOT NULL DEFAULT '',
    created_at BIGINT NOT NULL,
    read_at BIGINT
);
CREATE INDEX inbox_to ON inbox(to_user, created_at);

CREATE SEQUENCE shared_seq;
CREATE TABLE shared_playlists (
    id UUID PRIMARY KEY,
    owner UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    seq BIGINT NOT NULL               -- bumped on title/member changes
);
CREATE TABLE shared_playlist_members (
    playlist_id UUID NOT NULL REFERENCES shared_playlists(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    added_at BIGINT NOT NULL,
    PRIMARY KEY (playlist_id, user_id)
);
CREATE INDEX shared_playlist_members_user ON shared_playlist_members(user_id);
CREATE TABLE shared_playlist_entries (
    playlist_id UUID NOT NULL REFERENCES shared_playlists(id) ON DELETE CASCADE,
    entry_id UUID NOT NULL,
    value JSONB,                      -- {position, media}; NULL is a tombstone
    clock BIGINT NOT NULL,
    device_id UUID NOT NULL,
    user_id UUID NOT NULL,            -- author of the winning write (no FK: survives removal)
    seq BIGINT NOT NULL,
    PRIMARY KEY (playlist_id, entry_id)
);
CREATE INDEX shared_playlist_entries_seq ON shared_playlist_entries(playlist_id, seq);
CREATE TABLE shared_applied (
    playlist_id UUID NOT NULL REFERENCES shared_playlists(id) ON DELETE CASCADE,
    op_id UUID NOT NULL,
    applied_at BIGINT NOT NULL,
    PRIMARY KEY (playlist_id, op_id)
);
```

Jam tables are in §5.1 (same migration). The owner is also a row in
`shared_playlist_members`.

### 2.2 Friends

| Method, path | Body | Response | Rules |
| --- | --- | --- | --- |
| `GET /api/friends` | — | `{"friends":[{"user_id","username","since"}],"incoming":[{"user_id","username","at"}],"outgoing":[{"user_id","username","at"}]}` | sorted by username |
| `POST /api/friends/requests` | `{"username":"bob"}` | `{"status":"requested"}` or `{"status":"accepted"}` (the other side had already asked you) | normalize with `auth::normalize_username`; unknown, inactive or self → 404 `No such user`; already friends → 200 `{"status":"accepted"}`; throttle `FRIEND_REQUEST` 20 / user / hour (recorded on every call); max 100 friends (409), max 50 outgoing pending (409) |
| `POST /api/friends/{user_id}/accept` | — | `{"status":"accepted"}` | only the non-requesting side of a pending row; else 404 |
| `DELETE /api/friends/{user_id}` | — | `{"removed":true}` | removes pending (decline/cancel) or accepted row; on accepted removal also: remove each from the other's shared playlists (`shared_playlist_members`), remove each from Jam rooms hosted by the other, delete pending Jam invites between them. Inbox items stay. |

WebSocket events (to the affected user's channel, §4): `friend_request {"from":{"user_id","username"}}`,
`friend_added {"user_id","username"}` (to both), `friend_removed {"user_id"}` (to both).

### 2.3 Activity sharing

| Method, path | Body | Response |
| --- | --- | --- |
| `GET /api/social/settings` | — | `{"share_activity":"off"}` (missing row = `off`) |
| `PUT /api/social/settings` | `{"share_activity":"off"\|"now_playing"\|"history"}` | same; other values 400 |
| `GET /api/friends/activity` | — | `{"activity":[FriendActivity]}` |

`FriendActivity` (only friends whose `share_activity` ≠ `off`; others are omitted
entirely):

```json
{"user_id":"…","username":"bob",
 "now_playing":{"media":{…slim Media…},"playing":true,"position_ms":51000,"server_at":1763900000000},
 "recent":[{"title":"…","creator":"…","url":"https://…","kind":"song","thumbnail":"https://…","at":1763890000000}]}
```

- `now_playing`: the most recently updated presence (§4.3) among the friend's devices
  with non-null media, or `null`. Presence is in memory, so it is `null` after a
  restart until the friend's device reports again.
- `recent`: present only for `history`; last 20 `history` sync records of the friend
  with `at` in the last 7 days, newest first, only the six fields shown.
- Event `friend_activity {"user_id","now_playing"}` (same `now_playing` shape or
  `null`) goes to every friend of an opted-in user when that user's now-playing
  **media key or playing flag** changes, at most once per 10 s per sharing user
  (later changes inside the window are coalesced into one event at window end, or
  simply dropped — the friend sees the next one; document which).

### 2.4 Send to a friend (inbox)

| Method, path | Body | Response | Rules |
| --- | --- | --- | --- |
| `POST /api/inbox` | `{"to":"<user_id>","media":{Media},"note":"…"}` | 201 `{"id":"…"}` | `to` must be a friend (else 404 `No such friend`); media online, stored slim; `note` ≤ 200 chars, control chars rejected, `clean_text` applied; throttle `INBOX_SEND` 60 / sender / hour; recipient cap 200 rows: delete the recipient's oldest **read** items to make room, if still 200 → 409 `Their inbox is full` |
| `GET /api/inbox` | query `unread_only=true` optional | `{"items":[{"id","from":{"user_id","username"},"media","note","created_at","read"}],"unread":3}` newest first, max 200 | own inbox only |
| `POST /api/inbox/{id}/read` | — | `{"read":true}` | own items, idempotent |
| `DELETE /api/inbox/{id}` | — | `{"deleted":true}` | own items |

Event to recipient: `inbox_item {"id","from":{"user_id","username"}}`. Retention:
`maintenance` deletes items older than 90 days.

### 2.5 Collaborative (shared) playlists

Shared playlists are server-side objects, separate from the per-user `playlists` /
`playlist_entries` sync collections. Entries use the **same register model** as
`playlist_entries`: one register per entry, value `{"position": <fractional index>, "media": Media}`
(validation identical to the `PlaylistEntries` arm in `sync::validate`: position
`[0-9A-Za-z]{1,64}`, online slim media), later clock wins, device id breaks ties
(`sync::wins`), clocks clamped with `sync::clamp_clock`, op ids idempotent per playlist.

| Method, path | Body | Response | Rules |
| --- | --- | --- | --- |
| `POST /api/shared-playlists` | `{"id":"<client uuid>","title":"…"}` | 201 `SharedPlaylist` | title rules as `sync` playlists; a user may be a member of ≤ 50 shared playlists (409); same id again by the same owner → 200 with the existing one (idempotent create) |
| `GET /api/shared-playlists` | — | `{"playlists":[SharedPlaylist]}` | where caller is member |
| `PATCH /api/shared-playlists/{id}` | `{"title":"…"}` | `SharedPlaylist` | owner only (403) |
| `DELETE /api/shared-playlists/{id}` | — | `{"deleted":true}` | owner deletes; event `shared_playlist_removed {"playlist_id"}` to all members |
| `POST /api/shared-playlists/{id}/members` | `{"user_id":"…"}` | `SharedPlaylist` | owner only; target must be the owner's friend; ≤ 20 members incl. owner (409) |
| `DELETE /api/shared-playlists/{id}/members/{user_id}` | — | `{"removed":true}` | owner removes anyone but self; a member removes self (leave); removed user gets `shared_playlist_removed` |
| `POST /api/shared-playlists/{id}/push` | `{"ops":[{"id":"<op uuid>","entry_id":"<uuid>","value":{"position":"a0","media":{…}} \| null,"clock":1763900000000}]}` | `{"results":[{"id","status":"applied","seq":9}\|{"id","status":"superseded"}\|{"id","status":"duplicate"}\|{"id","status":"rejected","reason":"…"}],"seq":9}` | members only (404 for non-members, never 403, so ids are not probeable); ≤ 200 ops; ≤ 2000 live (non-tombstone) entries per playlist → per-op `rejected` "playlist is full"; serialize per playlist with `pg_advisory_xact_lock(hashtextextended(playlist_id::text, 1))` |
| `GET /api/shared-playlists/{id}/pull?since=0&limit=500` | — | `{"playlist":SharedPlaylist,"changes":[{"entry_id","value","clock","device_id","user_id","seq"}],"next":9,"more":false}` | members only; limit 1..=1000; same `since`/`next`/`more` semantics as `/api/sync/pull` |

`SharedPlaylist`:

```json
{"id":"…","title":"Road trip","owner":{"user_id":"…","username":"alice"},
 "members":[{"user_id":"…","username":"alice"},{"user_id":"…","username":"bob"}],
 "entry_count":12,"seq":41,"updated_at":1763900000000,"role":"owner"}
```

`seq` is the highest of the playlist's own `seq` and its entries' seq (one global
`shared_seq` sequence). `role` is `owner` or `member` for the caller.
After an applied push, publish `shared_playlist_changed {"playlist_id","seq"}` to every
member's user channel (also the pusher; clients ignore their own `device_id`, which the
event carries as `"device_id"`). Membership or title changes publish the same event.

Client model: see §8.3 (`LocalPlaylist.shared_id`). The client keeps one cursor per
shared playlist and pulls on `shared_playlist_changed` and on every `sync_now`.

---

## 3. B4 server: WebSocket client-to-server protocol (`presence.rs`, delivery rules in §4)

### 3.1 Frames, limits, errors

- Text frames only, one JSON object each, ≤ 32 KiB (`MAX_WS_MESSAGE`). Binary frames
  close the socket (code 1003).
- Every client message has `"type"` and optional `"rid"` (integer 1..=2^31-1, chosen by
  the client). Messages with `rid` get exactly one reply: `{"type":"ack","rid":n}` or
  `{"type":"error","rid":n,"code":"…","message":"…"}`. Messages without `rid` get a reply
  only for `ping` and `presence_query`, and an error only for `bad_message`.
- Error codes: `bad_message` (not JSON / wrong field types), `unknown_type`,
  `rate_limited`, `not_found` (target device not of this user / not connected),
  `forbidden`, `too_large`, `invalid` (validation, e.g. non-online media).
- Rate limits (in memory, per socket, token buckets; no database writes per message):
  `presence` 1 per 2 s (excess: `rate_limited`, state not stored); all other
  client messages together 20 per 10 s, burst 20. Three `bad_message`/`unknown_type`
  errors on one socket within 60 s close it (code 1008).
- Unknown extra fields are ignored. Strings are length-checked before use.

### 3.2 Messages (client → server)

| type | Fields | Server action |
| --- | --- | --- |
| `ping` | `t0` (i64, client ms) | reply `{"type":"pong","t0":t0,"server_ms":now}` immediately (no DB) |
| `presence` | `state`: PresenceState | store for this device, fan out §3.3 |
| `presence_query` | — | reply `{"type":"presence_snapshot","devices":[Presence]}` (all of this user's online devices except the caller) |
| `handoff` | `to_device`, `media` (online), `position_ms` (u64 ≤ 7 days), `playing` (bool), `queue` (≤ 25 Media, online) | target must be another online device of the **same user** (else `not_found`); forward §3.3; ack |
| `remote` | `to_device`, `command` ∈ `play` `pause` `toggle` `seek` `next` `prev` `volume`, `position_ms` (required for `seek`), `volume` (0..=100, required for `volume`) | same-user online target; forward; ack |

`PresenceState`:

```json
{"media":{…Media…} ,"playing":true,"position_ms":51000,"duration_ms":215000,
 "rate":1.0,"volume":80,"queue_len":4,"at":1763900000000,"can":["remote","handoff"]}
```

`media` may be `null` (idle). `media` must be online when present (local files are not
announced; the client sends `null` for them). The server stores the slim form.
`rate` 0.25..=4.0, `volume` 0..=100, `queue_len` ≤ 10 000, `can` ⊆ {`remote`,`handoff`}.

### 3.3 Events (server → client) for presence, handoff, remote

All server events are JSON objects with `"type"`. B1 events stay unchanged.

| type | Fields | Delivered to |
| --- | --- | --- |
| `presence` | `device_id`, `device_name`, `state` (PresenceState), `server_at` | the user's other sockets |
| `presence_gone` | `device_id` | the user's other sockets, when the device's last socket closes (or is revoked) |
| `presence_snapshot` | `devices`: [`{device_id, device_name, state, server_at}`] | reply only |
| `handoff` | `from_device`, `from_name`, `media`, `position_ms`, `playing`, `queue`, `to_device` | only sockets of `to_device` |
| `remote` | `from_device`, `from_name`, `command`, `position_ms?`, `volume?`, `to_device` | only sockets of `to_device` |
| `pong`, `ack`, `error` | see §3.1 | reply only |

### 3.4 Server implementation notes (`api.rs` changes by server-b4)

- `Hub::publish(user, event)`: if `event["to_device"]` is a UUID, `ws_session` delivers
  it only when it equals its own device. Hub broadcast capacity becomes 256.
- `ws_session` incoming arm: `Message::Text(t)` → `crate::presence::on_message(&state, &device, &t)`
  returns `Option<Value>` reply to send on this socket; `Message::Close`/error ends the
  session; ping/pong as before; binary closes.
- On session start: `presence::connected(&state, &device)`; on any exit path:
  `presence::disconnected(&state, &device)` (drop guard), which removes the device's
  presence when its last socket closes and publishes `presence_gone`.
- `Lagged` arm: send `{"type":"changed","seq":…,"resync":true}`; clients then pull and
  refetch presence and any open Jam room.
- `Realtime` (in `presence.rs`, `Default`): device → (user, name, socket count,
  PresenceState, server_at), per-socket buckets, last friend-activity emit per user.
  Device names are read once per socket open from `devices`.

---

## 4. Event delivery across users

Social and Jam events for **another** user use `state.0.hub.publish(other_user, event)`
(the B1 per-user channel). Every event carries enough ids to be applied idempotently.
Events never contain tokens, passwords, cookies or provider URLs other than media URLs.

---

## 5. B4 server: Jam (`jam.rs`)

Coordinate-only: the server stores queue and playback state and keeps the clock; it never
fetches, resolves or proxies media.

### 5.1 Tables (in `0004_social_realtime.sql`)

```sql
CREATE TABLE jam_rooms (
    id UUID PRIMARY KEY,
    host UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    guests_can_control BOOLEAN NOT NULL DEFAULT FALSE,
    playback JSONB NOT NULL,          -- JamPlayback (§5.3)
    version BIGINT NOT NULL DEFAULT 1,-- bumped on every change of anything in the room
    created_at BIGINT NOT NULL,
    last_active BIGINT NOT NULL,
    ended_at BIGINT
);
CREATE TABLE jam_members (
    room_id UUID NOT NULL REFERENCES jam_rooms(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('host','guest')),
    joined_at BIGINT NOT NULL,
    PRIMARY KEY (room_id, user_id)
);
CREATE INDEX jam_members_user ON jam_members(user_id);
CREATE TABLE jam_invites (
    room_id UUID NOT NULL REFERENCES jam_rooms(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    invited_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at BIGINT NOT NULL,
    PRIMARY KEY (room_id, user_id)
);
CREATE TABLE jam_queue (
    room_id UUID NOT NULL REFERENCES jam_rooms(id) ON DELETE CASCADE,
    item_id UUID NOT NULL,
    media JSONB NOT NULL,             -- slim online Media
    added_by UUID NOT NULL,
    added_at BIGINT NOT NULL,
    PRIMARY KEY (room_id, item_id)
);
CREATE TABLE jam_votes (
    room_id UUID NOT NULL,
    item_id UUID NOT NULL,
    user_id UUID NOT NULL,
    PRIMARY KEY (room_id, item_id, user_id),
    FOREIGN KEY (room_id, item_id) REFERENCES jam_queue(room_id, item_id) ON DELETE CASCADE
);
```

### 5.2 Limits and lifetime

`MAX_JAM_MEMBERS = 10` (host included), `MAX_JAM_QUEUE = 200`, `MAX_JAM_INVITES = 20`
open per room, a user hosts ≤ 1 active room and is a member of ≤ 1 active room (joining
another room leaves the current one; hosting a new one ends the old one).
Throttles: `JAM_ADD` 30 queue adds / user / 10 min, `JAM_CONTROL` 60 playback actions /
user / min, `JAM_CREATE` 10 rooms / user / hour. A room **ends** when the host ends or
leaves it, when no member has had an open WebSocket for 30 min (`last_active` is bumped
by any room action and by `jam_ping` §5.5), or 12 h after creation. Ended rooms are
answered with 404 and deleted by `maintenance` after 1 day. Expiry is checked lazily on
every access and by `maintenance`; ending publishes `jam_ended`.

### 5.3 Shapes

`JamPlayback`:

```json
{"item":{"item_id":"…","media":{…slim Media…},"media_key":"youtube:abc","added_by":"<user_id>"} ,
 "state":"playing","position_ms":51000,"at_server_ms":1763900000000,"rate":1.0,"version":17}
```

`item` is `null` with `state` = `"stopped"` when nothing is current. `state` ∈
`playing`, `paused`, `stopped`. Expected position at server time `T`:
`position_ms + (T - at_server_ms) * rate` when playing, else `position_ms`.
`version` equals the room's `version` at the time of the change.

`JamRoom` (GET response):

```json
{"id":"…","title":"Friday","host":{"user_id":"…","username":"alice"},
 "guests_can_control":false,"version":17,"created_at":…,"role":"guest",
 "members":[{"user_id":"…","username":"alice","role":"host","online":true}],
 "invited":[{"user_id":"…","username":"carol"}],
 "queue":[{"item_id":"…","media":{…},"media_key":"…","added_by":{"user_id","username"},"added_at":…,"votes":2,"voted":true}],
 "playback":{JamPlayback},"server_ms":1763900000123}
```

Queue order: `votes` desc, then `added_at` asc, then `item_id`. The current item is not
in `queue`. `online` = the member has ≥ 1 open socket (`Realtime`).

### 5.4 Endpoints

| Method, path | Body | Response | Rules |
| --- | --- | --- | --- |
| `POST /api/jam` | `{"title":"…"?,"guests_can_control":false?}` | 201 `JamRoom` | title default `"<username>'s Jam"`, ≤ 80 chars; ends caller's hosted room; leaves caller's current room |
| `GET /api/jam` | — | `{"current":JamRoom\|null,"invites":[{"room_id","title","from":{"user_id","username"},"created_at"}]}` | |
| `GET /api/jam/{id}` | — | `JamRoom` | members only, else 404 |
| `PATCH /api/jam/{id}` | `{"title"?,"guests_can_control"?}` | `JamRoom` | host only (403) |
| `DELETE /api/jam/{id}` | — | `{"ended":true}` | host only |
| `POST /api/jam/{id}/invite` | `{"user_ids":["…"]}` (≤ 10) | `{"invited":["…"]}` | any member may invite **their own friends**; non-friends/unknown silently skipped (not listed) ; event `jam_invite` to each |
| `POST /api/jam/{id}/join` | — | `JamRoom` | needs an invite row (deleted on join); full → 409 |
| `POST /api/jam/{id}/decline` | — | `{"declined":true}` | deletes own invite |
| `POST /api/jam/{id}/leave` | — | `{"left":true}` | host leaving ends the room |
| `DELETE /api/jam/{id}/members/{user_id}` | — | `{"removed":true}` | host only; kicked user gets `jam_ended {"room_id","reason":"removed"}` |
| `POST /api/jam/{id}/queue` | `{"media":{Media}}` | 201 queue item | any member; online media; queue full → 409; if nothing is current (`stopped`), the item becomes current immediately and playback goes `playing` at 0 |
| `DELETE /api/jam/{id}/queue/{item_id}` | — | `{"removed":true}` | host or the adder |
| `POST /api/jam/{id}/queue/{item_id}/vote` | `{"up":true}` | `{"votes":3,"voted":true}` | any member; `up:false` removes own vote; adder may vote |
| `POST /api/jam/{id}/playback` | JamAction | `JamPlayback` | permission below |

`JamAction` (`action` required):

| action | Fields | Effect |
| --- | --- | --- |
| `play` | `position_ms?` | `playing`; position = given or current expected position; `at_server_ms` = now |
| `pause` | `position_ms?` | `paused` at given or expected position |
| `seek` | `position_ms` | keeps state, new position |
| `next` | — | top queue item becomes current at 0, `playing`; empty queue → `stopped`, `item` null |
| `play_item` | `item_id` | that queue item becomes current at 0 (removed from queue) |
| `ended` | `item_id` (the current item's id) | same as `next` **only if** `item_id` is still current; otherwise no change and 200 with current playback (idempotent; every member's device sends it, first wins) |
| `rate` | `rate` (0.5..=2.0) | changes rate, re-anchors position |

Permission: host always; guests only when `guests_can_control`, except `ended`, which
any member may send. Otherwise 403 `Only the host can control playback`.
`position_ms` must be ≤ media duration (when known, in ms) and ≤ 24 h.
Each accepted action: bump `version`, store playback, publish `jam_state`.

### 5.5 Jam WebSocket events and clock

Server → client (to every member's user channel; non-members never receive room data):

| type | Fields |
| --- | --- |
| `jam_state` | `room_id`, `playback` (JamPlayback), `server_ms` |
| `jam_changed` | `room_id`, `version`, `parts` ⊆ [`queue`,`members`,`settings`] — client refetches `GET /api/jam/{id}` |
| `jam_invite` | `room_id`, `title`, `from` {user_id, username} |
| `jam_ended` | `room_id`, `reason` ∈ `ended`,`expired`,`removed` |

Client → server: `ping`/`pong` of §3.2 is the clock protocol (no separate Jam ping). A
member device in a room additionally sends `{"type":"jam_ping","room_id":"…"}` every
60 s (no reply), which bumps `last_active` (at most one DB write per room per 30 s).

Clock sync (client side, `vidcue_sync::clock`): send `ping{t0}`; on `pong` at local
`t1`, `rtt = t1 - t0`, `offset = server_ms - (t0 + rtt / 2)`. On joining a room take 5
samples 200 ms apart, discard samples with `rtt > 1000`, keep the offset of the sample
with the smallest rtt; resample (3 samples) every 60 s and after reconnect. Server time
now = `local_ms + offset`.

Drift rule (client, run on every `jam_state` and every 2 s while in a room):
`expected = position at server_now` (§5.3); `drift = player_position - expected`.
If the current media key differs → start that media at `expected`. If state differs →
play/pause. If `|drift| > 400 ms` → seek to `expected + 150 ms` (seek latency
allowance, constant `SEEK_LEAD_MS`). Otherwise nothing. Two seeks within 3 s are not
issued (backoff), so slow buffering cannot loop.

---

## 6. B5 server

### 6.1 Tables (`0005_personal.sql`)

```sql
ALTER TABLE users ADD COLUMN disabled_at BIGINT;

CREATE TABLE health_reports (
    id BIGSERIAL PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id UUID NOT NULL,
    kind TEXT NOT NULL,
    category TEXT NOT NULL,
    client_version TEXT NOT NULL,
    platform TEXT NOT NULL,
    at BIGINT NOT NULL,
    duration_ms INT,
    received_at BIGINT NOT NULL
);
CREATE INDEX health_reports_user_at ON health_reports(user_id, at);
CREATE INDEX health_reports_at ON health_reports(at);

-- Stats read history events by their own `at`; `::numeric` never fails for a JSON number.
CREATE INDEX sync_records_history_at ON sync_records (user_id, ((value->>'at')::numeric))
    WHERE collection = 'history' AND value IS NOT NULL;

CREATE TABLE stats_wrapped (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    year INT NOT NULL,
    tz_offset_min INT NOT NULL,
    computed_at BIGINT NOT NULL,
    value JSONB NOT NULL,
    PRIMARY KEY (user_id, year, tz_offset_min)
);

CREATE TABLE recs_settings (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    music BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at BIGINT NOT NULL
);
CREATE TABLE recs_feed (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    built_at BIGINT NOT NULL,
    items JSONB NOT NULL
);
-- Public upstream responses shared by all users (no user data in keys or values).
CREATE TABLE recs_cache (
    key TEXT PRIMARY KEY,
    value JSONB NOT NULL,
    fetched_at BIGINT NOT NULL,
    expires_at BIGINT NOT NULL
);

CREATE TABLE admin_sessions (
    token_hash BYTEA PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    csrf TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    last_used BIGINT NOT NULL,
    expires_at BIGINT NOT NULL
);
```

### 6.2 Playback health log (`health.rs`)

Sanitized categories only: no free text, URLs, ids, titles, cookies or error messages are
accepted or stored. The client classifies locally (§7.7).

`kind` ∈ `resolve` (stream resolution), `start` (first audio/frame), `stall`
(rebuffering during playback), `error` (fatal playback error), `download`.

`category` ∈ `ok`, `network_timeout`, `network_unreachable`, `tls`, `http_403`,
`http_404`, `http_429`, `http_5xx`, `extractor_failed`, `js_challenge_failed`,
`signature_failed`, `format_unavailable`, `members_only`, `age_restricted`,
`geo_blocked`, `drm`, `live_unsupported`, `decoder_error`, `audio_output_error`,
`offline_file_missing`, `storage_full`, `cancelled`, `unknown`.

`platform` ∈ `android`, `desktop`. `client_version` matches `^[0-9A-Za-z.+-]{1,40}$`.

| Method, path | Body | Response | Rules |
| --- | --- | --- | --- |
| `POST /api/health/reports` | `{"reports":[{"kind":"start","category":"ok","client_version":"0.2.0-beta.3","platform":"android","at":1763900000000,"duration_ms":840}]}` | `{"accepted":12,"rejected":1}` | ≤ 100 reports per request (else 400); a report with any unknown field, unknown enum value, `at` outside `[now-30 d, now+5 min]` or `duration_ms` outside 0..=600000 is rejected individually; ≤ 2000 accepted reports per user per rolling 24 h (excess rejected); throttle `HEALTH_POST` 30 requests / device / hour |
| `GET /api/health/summary?days=7` | — | HealthSummary | days 1..=90, own reports |
| `GET /api/admin/health?days=7` | — | HealthSummary + `"users_reporting":n,"by_version":[{"client_version","total","ok_rate"}]` | admin |

HealthSummary:

```json
{"days":7,"total":412,
 "by_kind":{"start":{"total":200,"ok":191,"ok_rate":0.955},"resolve":{…},"stall":{"total":9},"error":{"total":4},"download":{…}},
 "top_categories":[{"kind":"start","category":"http_403","count":5}],
 "start_ms":{"p50":820,"p90":2100},
 "by_day":[{"day":"2026-11-23","total":60,"ok":58}]}
```

`ok_rate` = ok / total for `resolve`, `start`, `download`; `stall`/`error` only count.
Percentiles from `start` reports with `duration_ms` and category `ok`. Days are UTC.
Retention: `maintenance` deletes reports with `at < now - 90 d`.

### 6.3 Stats and yearly review (`stats.rs`)

Input: the user's own `history` sync records (B3 clients write them per §8.3). A
history event value has these fields (server already validates types):

| Field | Type | Meaning |
| --- | --- | --- |
| `url` | string | online media URL (required) |
| `title`, `creator`, `thumbnail` | string ≤ 512 chars | display |
| `kind` | string | lowercase `MediaKind`: `video`, `song`, `playlist`, `album`, `artist`, `channel`, `station` |
| `at` | integer ms | when playback started (required for stats; events without it are ignored) |
| `seconds` | number | seconds actually listened/watched in this play (approximated as the furthest position reached) |
| `duration` | number | media length in seconds, if known |
| `finished` | bool | reached the end |

Stats ignore events with `seconds < 5` (accidental starts) and clamp `seconds` to
`duration` when present and to 6 h otherwise. Time zone: `tz_offset_min`
(-840..=840, minutes east of UTC) shifts day/hour bucketing.

| Method, path | Response |
| --- | --- |
| `GET /api/stats?from=<ms>&to=<ms>&tz_offset_min=60` | PeriodStats; `to - from` ≤ 366 days, `from < to`, else 400 |
| `GET /api/stats/wrapped?year=2026&tz_offset_min=60` | Wrapped; `year` 2020..=current; cached in `stats_wrapped` and recomputed when older than 6 h (past years: cached forever once computed after Jan 7 of the next year) |

PeriodStats:

```json
{"from":…,"to":…,"plays":312,"seconds":64200.5,"distinct_items":201,"finished":180,
 "by_kind":{"video":{"plays":120,"seconds":40000.0},"song":{"plays":192,"seconds":24200.5}},
 "top_creators":[{"creator":"…","plays":40,"seconds":9000.0,"thumbnail":"https://…"}],
 "top_items":[{"url":"…","title":"…","creator":"…","thumbnail":"…","kind":"song","plays":12,"seconds":2400.0}],
 "by_day":[{"day":"2026-11-23","seconds":3600.0,"plays":12}],
 "by_hour":[0.0, … 24 numbers …]}
```

Top lists: 10 entries, ordered by seconds desc then plays desc. `thumbnail` of a
creator = thumbnail of their most played item. `by_day` includes only days with plays.

Wrapped:

```json
{"year":2026,"seconds":512000.0,"plays":4100,"distinct_items":2100,
 "top_creators":[…5…],"top_items":[…5…],"top_songs":[…5, kind song…],"top_videos":[…5, kind video…],
 "by_month":[{"month":1,"seconds":…}, … 12 …],
 "busiest_day":{"day":"2026-08-14","seconds":21000.0},
 "longest_streak_days":23,"first_play":{"title":"…","creator":"…","at":…},
 "new_creators":140,"music_share":0.47,"computed_at":…}
```

`new_creators` = creators whose first play ever (all history) is in that year.
`music_share` = song seconds / all seconds. Empty year → all zero/empty, never 404.

### 6.4 Private recommendations (`recs.rs`)

Opt-in per user (default **off**) and globally switchable:
`VIDCUE_RECS` = `on` (default) | `off`; `VIDCUE_RECS_MUSIC` = `on` (default) | `off`
(ListenBrainz/MusicBrainz lookups); `VIDCUE_RECS_FETCH_PER_HOUR` (default 120, global
budget of upstream requests across all users). With `VIDCUE_RECS=off`, all `/api/recs*`
endpoints answer 404 and `/api/features` reports `recs_available:false`.

| Method, path | Body | Response |
| --- | --- | --- |
| `GET /api/recs/settings` | — | `{"enabled":false,"music":true}` |
| `PUT /api/recs/settings` | `{"enabled":true,"music":true}` | same; disabling deletes `recs_feed` for the user |
| `GET /api/recs` | — | `{"enabled":true,"built_at":…\|null,"building":false,"items":[RecItem]}`; disabled → `{"enabled":false,"items":[]}` |
| `POST /api/recs/refresh` | — | 202 `{"building":true}`; 429 if the last build started < 30 min ago |

`RecItem`: `{"media":{…slim Media…},"reason":"subscription"|"related"|"similar_artist","source":"<channel title, played item title or artist name>"}`.

Build (background task, at most one build at a time server-wide, per-user advisory
lock; `GET` triggers a build when enabled and `built_at` is older than 6 h):

1. **subscriptions**: up to 8 channels from the user's `subscriptions` collection
   (rotate: least recently used first, remembered in `recs_cache` key
   `rotation:<user>` — the only per-user cache row, never shared) → channel uploads →
   up to 3 newest per channel.
2. **related**: the 5 most recent `history` events with `finished:true` (or
   `seconds ≥ 60`) → related items (YouTube Music related for `kind:"song"`) → up to 4 each.
3. **similar_artist** (if `VIDCUE_RECS_MUSIC` and user `music`): top 3 song creators of
   the last 30 days → MusicBrainz artist MBID → ListenBrainz similar artists → top 5
   names not already among the user's creators → YouTube Music song search → 1 song each.
4. Drop items whose media key is in the user's history (any) or `saved`, items matching
   the user's `filters` (§8.5 semantics, server copy), duplicates; interleave sources
   round-robin; cap 60; store in `recs_feed`.

Upstream access, all through traits so tests run offline:

```rust
#[async_trait::async_trait]
pub trait PublicMedia: Send + Sync {
    async fn channel_uploads(&self, channel_id: &str) -> anyhow::Result<Vec<Media>>;
    async fn related(&self, video_id: &str, music: bool) -> anyhow::Result<Vec<Media>>;
    async fn search_songs(&self, query: &str) -> anyhow::Result<Vec<Media>>;
}
#[async_trait::async_trait]
pub trait MusicSimilarity: Send + Sync {
    async fn artist_mbid(&self, name: &str) -> anyhow::Result<Option<String>>;
    async fn similar_artists(&self, mbid: &str) -> anyhow::Result<Vec<(String /*name*/, String /*mbid*/)>>;
}
```

- Production `PublicMedia` = `vidcue_providers::browse::Browser::new(client, None, 0)`
  (**no session, never the user's cookies**; the server has none) with
  `vidcue_providers::http_client(None)`: `fetch(false, &BrowseRequest::Collection(channel_id))`
  for uploads (verify against the provider's channel tests; if the channel page needs a
  different request, use what the app uses for a channel tap), `fetch(music, &BrowseRequest::Related(video_id))`,
  `fetch(true, &BrowseRequest::MusicSearch { query, category: "songs".into() })`.
  Only online, non-live items with a YouTube media key are kept.
- Production `MusicSimilarity`: MusicBrainz `GET https://musicbrainz.org/ws/2/artist?query=artist:"<name>"&limit=1&fmt=json`
  (take the first artist with `score ≥ 90`); ListenBrainz
  `GET https://labs.api.listenbrainz.org/similar-artists/json?artist_mbids=<mbid>&algorithm=<ALGORITHM>`
  with `ALGORITHM` a `const` (`session_based_days_7500_session_300_contribution_5_threshold_10_limit_100_filter_True_skip_30`);
  parse defensively (unknown shape → empty, logged at debug). `User-Agent: vidcue-server/<version> (self-hosted)`
  — no public URL, username or contact data.
- Rate limits: MusicBrainz ≤ 1 request/s, ListenBrainz ≤ 1 request/s, YouTube ≤ 1
  request/s, plus the global hourly budget; a build that runs out of budget stores what
  it has. Request timeout 10 s, response body ≤ 2 MiB (`limited_text`).
- Cache (`recs_cache`, keys `yt:uploads:<channel_id>` 6 h, `yt:related:<id>:<music>` 24 h,
  `yt:songs:<lowercased query>` 24 h, `mb:artist:<lowercased name>` 30 d,
  `lb:similar:<mbid>` 7 d); values are slim Media arrays or parsed results only.
  `maintenance` deletes expired rows and caps the table at 50 000 rows (oldest first).
- Fixtures: `apps/server/tests/fixtures/recs/{musicbrainz_artist.json,listenbrainz_similar.json}`
  (hand-made, small) for the parsers; `PublicMedia`/`MusicSimilarity` fakes for builds.

### 6.5 Admin: JSON API (for the app) and web page (`admin_web.rs`)

JSON (Bearer, admin only; existing `POST /api/admin/invites`, `GET /api/admin/users`,
`POST /api/admin/users/{id}/reset` stay as they are):

| Method, path | Body | Response | Rules |
| --- | --- | --- | --- |
| `GET /api/admin/users/{id}/devices` | — | `{"devices":[{"id","name","created_at","last_seen","paired_by"}]}` | |
| `POST /api/admin/users/{id}/devices/revoke` | `{"password","device_id":"…"\|null}` | `{"revoked":["…"]}` | `reauth(password)`; null = all devices of that user (uses `revoke_tree` per root, publishes `devices_revoked`/`device_revoked`) |
| `POST /api/admin/users/{id}/disable` | `{"password"}` | `{"disabled":true}` | `reauth(password)`; not self (409), not the last enabled admin (409); sets `disabled_at`, revokes all devices, deletes the user's open pairings, resets, key requests and admin sessions, publishes `devices_revoked` |
| `POST /api/admin/users/{id}/enable` | `{"password"}` | `{"disabled":false}` | `reauth(password)`; clears `disabled_at` (the user signs in again) |
| `GET /api/admin/health?days=7` | — | §6.2 | |

Web page at `/admin` (server-rendered HTML, **no JavaScript**, forms only):

- `GET /admin` → login form (no session) or dashboard (valid session).
- `POST /admin/login` (`application/x-www-form-urlencoded`: `username`, `password`,
  `code`) → same throttles as `/api/login` with keys prefixed `admin-` (`LOGIN_IP`,
  `LOGIN_USER_IP`, `LOGIN_USER`, then `SECOND_FACTOR` after a correct password),
  `argon()`, `dummy_verify` for unknown users. Non-admin or disabled users get the same
  generic failure as a wrong password. Success: 303 to `/admin` with
  `Set-Cookie: vidcue_admin=<vca_ token>; Path=/admin; HttpOnly; Secure; SameSite=Strict; Max-Age=1800`.
  Token random 32 bytes, stored as SHA-256 only; idle timeout 30 min (`last_used`),
  absolute 8 h; ≤ 3 sessions per admin (oldest deleted).
- CSRF: every POST form except login carries hidden `csrf` (per-session random,
  constant-time compare); additionally reject POSTs whose `Origin` header is present
  and differs from the origin of `VIDCUE_PUBLIC_URL`, and whose `Sec-Fetch-Site` is
  present and not `same-origin`. Failure → 403 page, nothing changes.
- Headers like `web::page_response` but with this CSP:
  `default-src 'none'; style-src '<sha256 of the inline <style>>'; img-src data:; form-action 'self'; base-uri 'none'; frame-ancestors 'none'`;
  `Cache-Control: no-store`, `Referrer-Policy: no-referrer`, `X-Content-Type-Options: nosniff`.
  All dynamic text HTML-escaped (one escape function with a unit test).
- Dashboard sections: users table (username, admin, devices, last seen, disabled) with
  per-user forms: *reset link* (password + code), *revoke all devices* (password),
  *disable/enable* (password); *create invite* (password, admin checkbox, hours) showing
  the link once plus its QR as inline `<svg>` markup from `api::qr_svg` (inline SVG
  needs no `img-src`); health aggregate (§6.2, 7 days) as a plain table;
  `POST /admin/logout` deletes the session.
- The page never displays device tokens, TOTP secrets or recovery codes.
- `maintenance` deletes expired admin sessions.

---

## 7. B3 client library: `crates/vidcue-sync` (owner client-core)

Pure Rust, no Android or JNI code, reusable by a future desktop app. `Cargo.toml`:
`vidcue-core` (path), `anyhow`, `serde`, `serde_json`, `tokio` (workspace),
`uuid` (workspace), `url` (workspace), `reqwest = { workspace = true, features = ["rustls-tls"] }`,
`tokio-tungstenite = { version = "0.29", default-features = false, features = ["connect", "__rustls-tls"] }`
— if that internal feature name differs in 0.29, use the smallest public rustls feature
and pass a `Connector::Rustls(Arc<rustls::ClientConfig>)` built with
`rustls = { version = "0.23", default-features = false, features = ["ring", "std", "tls12"] }`
and `webpki-roots = "1"`. **Gate:** `cargo tree -p vidcue-sync -i aws-lc-rs` must
report no match (Android NDK builds use `ring` only). Crypto: `chacha20poly1305 = "0.10"`,
`x25519-dalek = { version = "2", features = ["static_secrets"] }`, `hkdf = "0.12"`,
`sha2 = "0.10"`, `rand = "0.8"`, `data-encoding = "2"`, `futures-util = "0.3"`,
`qrcode = { version = "0.14", default-features = false }`.

### 7.1 Modules

| Module | Contents |
| --- | --- |
| `api` | `Api { base: Url, token: Option<String>, http: reqwest::Client }` with one async method per endpoint in §2, §5, §6 and B1/B2 (login, me, devices, revoke, pairing start/claim, push, pull, ws ticket, recovery codes, password, totp, key requests, admin). Errors: `ApiError { status: u16, message: String }`; `401` maps to `ApiError::Unauthorized`. Timeouts 20 s; body ≤ 8 MiB. Never logs bodies or headers. |
| `store` | `trait SecretStore { fn load(&self, name: &str) -> Result<Option<String>>; fn save(&self, name: &str, value: &str) -> Result<()>; fn delete(&self, name: &str) -> Result<()>; }` (platform seals). `MemoryStore` for tests. Names: `session` (SessionInfo JSON), `e2e_key` (base64 key). Plain JSON state files under a directory the app passes: `state.json`, `outbox.json`, `health.json`. |
| `outbox` | persisted op queue (§7.3) |
| `engine` | `SyncEngine` push/pull loop and conflict application (§7.4) |
| `ws` | WebSocket client (§7.5) |
| `crypto` | secrets envelope, sealed box, fingerprint (§7.6) |
| `pairing` | URI build/parse (§7.6.4) |
| `fractional` | `between(lo: Option<&str>, hi: Option<&str>) -> String` (§7.8) |
| `clock` | `OffsetEstimator`, `expected_position`, `drift_action` (§5.5) |
| `protocol` | serde types: `PresenceState`, `ClientMessage`, `ServerEvent` (all §3/§5 types, `#[serde(tag = "type", rename_all = "snake_case")]` with an `Other` fallback that keeps unknown events), `JamPlayback`, `JamRoom`, `SharedPlaylist`, `FriendActivity`, `HistoryEvent`, `HealthReport`, enums `HealthKind`, `HealthCategory` |
| `health` | `classify(error_text: &str) -> HealthCategory` (pure, table-driven; `unknown` fallback) and a bounded buffer (500 reports) |

`SessionInfo = {server: String (origin), user_id, username, device_id, is_admin: bool, token: String}`.
Server origin rules: `https://host[:port]` only, no path/query/fragment/userinfo;
`http` only for hosts `localhost`, `127.0.0.1`, `10.0.2.2` (emulator tests).

### 7.2 Local store interface the app implements

```rust
pub struct Change { pub collection: String, pub key: String, pub value: Option<serde_json::Value>, pub clock: i64, pub device_id: Uuid, pub seq: i64 }
pub trait LocalStores: Send + Sync {
    /// Apply one remote register value (None = deleted). Called in seq order.
    fn apply(&self, change: &Change) -> anyhow::Result<()>;
    /// Every register this device holds for a collection, for the first-sync union.
    fn snapshot(&self, collection: &str) -> anyhow::Result<Vec<(String /*key*/, serde_json::Value, i64 /*clock*/)>>;
    /// Called once after a pull batch was applied (UI refresh hook).
    fn applied(&self, collections: &[String]);
}
```

### 7.3 Outbox

`outbox.json` = `{"ops":[{"id","collection","key","value","clock","attempts":0,"next_at":0}]}`.
- `enqueue(collection, key, value, clock)` **coalesces**: a pending op for the same
  `(collection, key)` is replaced (new op id, new value, new clock). So the outbox is
  bounded by the number of registers touched; hard cap 20 000 ops (beyond it the oldest
  `history` ops are dropped first, then refuse with an error the UI shows once).
- Persisted atomically (write temp + rename) on every enqueue batch.
- Push: up to 500 ops per `POST /api/sync/push`. Per result: `applied`, `duplicate`,
  `superseded` → remove; `rejected` → remove and count in status
  (`rejected_count`, never the value). Network error, 5xx, 503 → keep, backoff
  `min(2^attempts × 2 s, 5 min)` ± 20 % jitter. 429 → 60 s. 401 → stop, state
  `signed_out`, reason `revoked` (keep local data and outbox; they push after the next
  sign-in to the same account, and are dropped when a different `user_id` signs in).
- Clocks: `max(now_ms, last_clock + 1)` per device so local edits order strictly.

### 7.4 Sync engine

`state.json` = `{"cursor":0,"first_sync_done":false,"collections":{…enabled flags…},
"shared_cursors":{"<playlist id>":0},"last_success":0,"last_error":null}`.

`sync_once()`:
1. If `first_sync_done` is false: pull everything (loop while `more`), apply, then for
   each enabled collection `snapshot()` and enqueue local registers that are missing
   remotely or whose local clock is newer (union; never deletes remote data); set
   `first_sync_done`.
2. Push the outbox (§7.3).
3. Pull from `cursor` with `limit=1000` while `more`; for each change: skip disabled
   collections; skip if an outbox op for the same register would win under
   `(clock, device_id)` ordering (it will be pushed); else `apply`. Advance and persist
   `cursor` after each page.
4. Pull each shared playlist whose server `seq` > local cursor (§2.5).
5. Flush health reports when enabled (§7.7).

Triggers: 2 s debounce after a local enqueue; WebSocket `changed` from another device
(`device_id` ≠ own) or with `resync`; `sync_now`; WorkManager (§8.6). One run at a time
(a mutex; a trigger during a run schedules exactly one follow-up run).

### 7.5 WebSocket client

- Connect `wss://<server>/api/ws` with `Authorization: Bearer <token>` (the native
  client can set headers; tickets are not needed). On `hello`: store `features`, compare
  `seq` with the cursor and run `sync_once` if newer.
- Reconnect with backoff 1 s, 2 s, 4 s … max 60 s, ± 20 % jitter; reset after 60 s
  connected. A close after revocation (`device_revoked` for own id / `devices_revoked`
  without `keep` = own id) or HTTP 401 on upgrade → signed out, no reconnect.
- The server sends WebSocket pings every 25 s; the client answers with pongs and sends
  no keepalive of its own. Client `ping` JSON messages exist only for the Jam clock.
- Outgoing messages go through a bounded channel (64); when full or disconnected,
  presence messages are dropped, `handoff`/`remote` fail with "device offline".
- Only sends client messages when `features` contains `realtime`.
- Exposes a `broadcast::Receiver<ServerEvent>` for the app.

### 7.6 End-to-end key

#### 7.6.1 Account key and key check

- Account key `K`: 32 random bytes (OS RNG). Stored only via `SecretStore` (`e2e_key`,
  standard base64).
- **Key check register**: collection `secrets`, key `keycheck`, value = envelope (§7.6.2)
  of the plaintext `vidcue-keycheck-v1` with AAD `vidcue/secrets/v1/keycheck`.
- After sign-in and first pull: if `secrets/keycheck` exists and opens with the local
  key → key OK. If it exists and no local key → key **missing** (request via relay
  §7.6.3, or arrives by QR). If it exists and does not open → key **mismatch** (same as
  missing; discard the local key after a successful relay). If it does not exist and the
  device has no key → generate `K` and push `keycheck`. A device that lost a keycheck
  race finds a mismatch at the next pull, obtains the winner's key by relay, then
  re-pushes its own provider session encrypted with the new key.
- Without a valid key the `secrets` collection is neither applied nor pushed
  (status `key_state:"missing"`).

#### 7.6.2 Secrets envelope (matches `sync::validate_secret`)

`{"v":1,"alg":"xchacha20poly1305","nonce":B64(24 random bytes),"ciphertext":B64(ct||tag)}`
where B64 is **standard base64 with padding** (`data_encoding::BASE64`), cipher
XChaCha20-Poly1305 with key `K`, AAD = ASCII `vidcue/secrets/v1/<register key>` (e.g.
`vidcue/secrets/v1/youtube`), plaintext = UTF-8 JSON. Plaintext ≤ 46 000 bytes (the
encoded value must stay under the server's 64 KiB); larger → error "session too large
to sync", nothing pushed.

#### 7.6.3 Key relay sealed box (`/api/keys/requests`)

Requester (device without `K`): ephemeral X25519 secret `r`, public `R`. Posts
`{"public_key": B64(R)}` (standard base64, 32 bytes). Keeps `r` in memory only (the
relay expires after 15 min; a restarted app makes a new request).

Granter (device with `K`), after the user compared fingerprints (§7.6.5):
1. ephemeral X25519 secret `e`, public `E`;
2. `s = X25519(e, R)`; abort if `s` is all zeros;
3. `okm = HKDF-SHA256(salt = E || R, ikm = s, info = "vidcue key relay v1", L = 32)`;
4. `n` = 24 random bytes; `c = XChaCha20-Poly1305(okm).encrypt(nonce = n, aad = E || R, plaintext = K)` (48 bytes);
5. `sealed = 0x01 || E (32) || n (24) || c (48)` = 105 bytes; post
   `{"sealed_key": B64(sealed)}` to `/api/keys/requests/{id}/grant`.

Requester on `key_granted` (or polling `GET /api/keys/requests/{id}` every 5 s for at
most 15 min): check version byte `0x01` and length 105, recompute `s = X25519(r, E)`,
derive `okm`, decrypt; the result must be 32 bytes and must open `secrets/keycheck`;
otherwise reject and show "key did not match".

#### 7.6.4 Pairing URI

Server returns `vidcue://pair?server=<percent-encoded origin>&code=<10 base32 chars>`.
The showing device appends `#k=<base64url without padding of K>` (43 chars) **locally**
and renders the QR from the full string; the fragment never goes to the server.
`parse_pairing_uri(s) -> Result<Pairing { server: String, code: String, key: Option<[u8; 32]> }>`:
scheme `vidcue`, host `pair`, exactly one `server` and `code`, `server` passes the
origin rules of §7.1, `code` matches `^[A-Z2-7]{10}$` after uppercasing, `k` decodes to
32 bytes when present; anything else is an error. URI ≤ 512 chars.

#### 7.6.5 Fingerprint

`fingerprint(public_key: &[u8; 32]) -> String`: `d = SHA-256("vidcue-fingerprint-v1" || public_key)`,
`n = u64::from_be_bytes(d[0..8]) % 10^12`, formatted as 12 zero-padded digits in four
groups of three: `"042 917 305 118"`.

**Deviation from the 6-digit draft, on purpose:** a malicious server that swaps in its
own public key must find a key whose fingerprint matches the one the requesting device
shows. Six digits (~20 bits) can be ground in well under a second; twelve digits
(~40 bits) cannot be ground within the 15-minute request lifetime on homelab hardware.

### 7.7 Health reports (client)

Enabled per device by the `health_reports` toggle (default **off**). The engine records
`resolve` outcomes itself (around the existing `resolve` op) and accepts `start`,
`stall`, `error`, `download` from the platform via `health_record`. Each report gets
`at = now`, `client_version` = app version, `platform` = `android`. Error text is
classified locally by `health::classify` and then discarded; it is never stored or sent.
Buffer ≤ 500 (oldest dropped), flushed in batches of ≤ 100 by `sync_once` when signed in.

### 7.8 Fractional positions

Alphabet `0-9A-Za-z` (ASCII order, base 62). `between(lo, hi)` returns a string `p` with
`lo < p < hi` in byte order (`None` = open end), shortest such string, never ending in
`0`, length ≤ 64. When the result would exceed 64 chars, the caller rebalances the whole
playlist (evenly spaced 2-char positions, one op per entry). Ties (equal positions from
concurrent inserts) order by entry id. Property tests: 10 000 random inserts keep order
and length limits.

---

## 8. B3 engine integration: `vidcue-mobile` (owner client-core)

### 8.1 Engine ops

All requests are `NativeBridge.request({"op": …, …})`; responses are the op's JSON
(errors as today: `anyhow` message, which the UI shows; messages never contain tokens,
passwords, cookies or the key). `sync_bridge::handles(op)` is true for every op below.
Ops marked **net** need network and may take seconds; the UI calls them from coroutines
with `request_id` for cancellation like other long ops.

**Account and sync**

| op | Request | Response |
| --- | --- | --- |
| `sync_status` | — | SyncStatus (§8.2) |
| `sync_sign_in` **net** | `server`, `username`, `password`, `code`, `device_name` | SyncStatus |
| `sync_sign_out` | `forget_data: bool` (false keeps local library) | SyncStatus — local only: deletes `session`/`e2e_key`/outbox/state, stops ws (does **not** revoke on the server; the UI offers "remove this device" via `sync_revoke` with own id first) |
| `sync_pair_start` **net** | `password` | `{"uri":"vidcue://pair?…#k=…","qr":["0101…",…],"expires_at":…}` — `qr` rows of `0`/`1` (1 = dark), quiet zone included (4 modules), EC level M; fails with "No account key yet" when the key state is not `ok` |
| `sync_pair_claim` **net** | `uri`, `device_name` | SyncStatus; stores `K` from `#k` (verified against `keycheck` after the first pull; mismatch → key state `mismatch`) |
| `sync_pair_inspect` | `uri` | `{"server":"https://…","host":"vidcue.example.org","has_key":true}` or error (for the confirm sheet before claiming) |
| `sync_devices` **net** | — | `{"devices":[{"id","name","created_at","last_seen","paired_by","current","online","now_playing":Media\|null}]}` (`online`/`now_playing` from presence) |
| `sync_revoke` **net** | `device_id` | `{"revoked":true}`; own id → also signs out locally |
| `sync_now` **net** | — | SyncStatus after one `sync_once` |
| `sync_settings` | `collections`: `{name: bool}` partial, `health_reports?: bool` | SyncStatus |
| `sync_foreground` | `visible: bool` | `{}` — visible: connect ws if signed in; hidden: close ws after 60 s unless audio is playing (`playing: bool` optional field keeps it open) or the device is in a Jam |
| `key_requests` **net** | — | `{"requests":[{"id","device_id","device_name","fingerprint","created_at"}]}` (fingerprint computed locally from the posted public key) |
| `key_grant` **net** | `id` | `{"granted":true}` (user confirmed the fingerprint in UI) |
| `key_request_start` **net** | — | `{"id","fingerprint","expires_at"}` (requester side; engine then waits for `key_granted`) |
| `key_fingerprint` | — | `{"state":"ok"\|"missing"\|"mismatch"\|"waiting","fingerprint":"…"\|null}` |
| `account_recovery_status` **net** | — | `{"remaining":7}` |
| `account_recovery_regenerate` **net** | `password`, `code` | `{"codes":["XXXXX-XXXXX",…]}` (shown once, never persisted) |
| `account_password` **net** | `current_password`, `code`, `new_password`, `sign_out_others` | `{"changed":true}` |
| `account_totp_start` **net** | `password`, `code` | `{"secret","otpauth_uri","qr":[rows]}` |
| `account_totp_confirm` **net** | `code` | `{"confirmed":true}` |

**Social** (`social_ops.rs`; all **net**; thin wrappers returning the server JSON of §2
unchanged unless noted)

| op | Request | Server call |
| --- | --- | --- |
| `friends` | — | `GET /api/friends` |
| `friend_request` | `username` | `POST /api/friends/requests` |
| `friend_accept` | `user_id` | `POST …/accept` |
| `friend_remove` | `user_id` | `DELETE /api/friends/{id}` |
| `friends_activity` | — | `GET /api/friends/activity` |
| `social_settings` | `share_activity?` | GET, or PUT when the field is given |
| `inbox` | `unread_only?` | `GET /api/inbox` |
| `inbox_send` | `to`, `media`, `note?` | `POST /api/inbox` |
| `inbox_read` / `inbox_delete` | `id` | POST read / DELETE |
| `shared_playlist_share` | `playlist_id` (local), `user_ids` | creates the server playlist with id = local playlist id if not shared yet, pushes all entries, adds members; marks local `shared_id` |
| `shared_playlist_leave` | `playlist_id` | owner: DELETE playlist (local copy becomes a normal playlist); member: remove self (local copy deleted) |
| `shared_playlist_members` | `playlist_id`, `add?: [user_id]`, `remove?: [user_id]` | member endpoints |

**Presence, handoff, remote**

| op | Request | Response |
| --- | --- | --- |
| `presence_update` | `media` (Media\|null), `playing`, `position_ms`, `duration_ms?`, `volume?`, `queue_len?` | `{}`; engine rate-limits to 1 per 2 s and only sends on change of media key/playing/seek (> 3 s jump) or every 30 s while playing; never sends local-file media |
| `presence_devices` | — | `{"devices":[{"device_id","device_name","state","server_at"}]}` (cached from events; sends `presence_query` when connected) |
| `handoff_send` | `to_device`, `media`, `position_ms`, `playing`, `queue` (≤ 25, trimmed) | `{"sent":true}` or error "device offline" |
| `remote_send` | `to_device`, `command`, `position_ms?`, `volume?` | `{"sent":true}` |

**Jam** (all **net** except `jam_target`)

| op | Request | Response |
| --- | --- | --- |
| `jam_create` | `title?`, `guests_can_control?` | JamRoom; starts clock sampling |
| `jam_current` | — | `GET /api/jam` |
| `jam_room` | `room_id` | JamRoom |
| `jam_update` | `room_id`, `title?`, `guests_can_control?` | JamRoom |
| `jam_end` / `jam_leave` / `jam_join` / `jam_decline` | `room_id` | server JSON; join starts clock sampling |
| `jam_invite` | `room_id`, `user_ids` | server JSON |
| `jam_kick` | `room_id`, `user_id` | server JSON |
| `jam_queue_add` | `room_id`, `media` | queue item |
| `jam_queue_remove` | `room_id`, `item_id` | server JSON |
| `jam_vote` | `room_id`, `item_id`, `up` | server JSON |
| `jam_control` | `room_id`, `action`, `position_ms?`, `item_id?`, `rate?` | JamPlayback |
| `jam_target` | `player_media_key?`, `player_position_ms?`, `player_playing?` | `{"in_room":true,"media":Media\|null,"media_key","playing":bool,"target_ms":51234,"action":"none"\|"load"\|"play"\|"pause"\|"seek","seek_ms":51384}` — pure local computation with the clock offset and drift rule of §5.5 |

**Personal and admin** (all **net**)

| op | Request | Server call / response |
| --- | --- | --- |
| `health_record` | `kind`, `category?` or `error_text?`, `duration_ms?` | local buffer only; `{}` |
| `health_summary` | `days?` | `GET /api/health/summary` |
| `stats` | `from`, `to`, `tz_offset_min` | `GET /api/stats` |
| `stats_wrapped` | `year`, `tz_offset_min` | `GET /api/stats/wrapped` |
| `recs` | — | `GET /api/recs` (items as Media cards with `reason`) |
| `recs_settings` | `enabled?`, `music?` | GET or PUT |
| `recs_refresh` | — | `POST /api/recs/refresh` |
| `admin_users` | — | `GET /api/admin/users` |
| `admin_invite` | `password`, `admin`, `hours` | POST invites; response adds `qr` rows (from the URL, locally) and drops `qr_svg` |
| `admin_reset` | `user_id`, `password`, `code` | reset link (+ `qr` rows) |
| `admin_devices` | `user_id` | GET devices |
| `admin_revoke` | `user_id`, `password`, `device_id?` | POST revoke |
| `admin_disable` / `admin_enable` | `user_id`, `password` | POST |
| `admin_health` | `days?` | GET |

**Local library ops (new stores, §8.3)**

| op | Request | Response |
| --- | --- | --- |
| `playlists` | — | `[{"id","title","count","shared":bool,"role":"owner"\|"member"\|null,"cover":Media\|null}]` |
| `playlist_create` | `title`, `media?` (first entry) | playlist |
| `playlist_rename` | `id`, `title` | playlist |
| `playlist_delete` | `id` | `{}` (tombstones header + entries) |
| `playlist_entries` | `id` | `[{"entry_id","media"}]` in order |
| `playlist_add` | `id`, `media` | `{"entry_id"}` (appended: `between(last, None)`) |
| `playlist_remove` | `id`, `entry_id` | `{}` |
| `playlist_move` | `id`, `entry_id`, `after_entry_id?` (null = to top) | `{}` |
| `follows` | — | `[{"channel_id","title","thumbnail"}]` |
| `follow_toggle` | `channel_id`, `title`, `thumbnail?` | `{"following":bool}` |
| `filters` | — | `[{"id","kind","value"}]` |
| `filter_add` | `kind`, `value?` | filter |
| `filter_remove` | `id` | `{}` |

Shared playlist entries change through the same `playlist_*` ops: when the local
playlist has `shared_id`, the engine queues ops for `/api/shared-playlists/{id}/push`
(a second outbox file `shared_outbox.json`, same semantics) instead of the personal
`playlist_entries` collection; the header of a shared playlist is not in the personal
`playlists` collection.

### 8.2 SyncStatus and events

```json
{"signed_in":true,"server":"https://vidcue.example.org","username":"alice","user_id":"…",
 "device_id":"…","is_admin":true,"state":"idle"|"syncing"|"offline"|"signed_out"|"error",
 "reason":null|"revoked"|"network"|"server_too_old",
 "connected":true,"features":["realtime","social","jam","health","stats","recs","admin_web"],
 "last_success":1763900000000,"pending":3,"rejected_count":0,
 "key_state":"ok"|"missing"|"mismatch"|"waiting",
 "collections":{"positions":true,"history":true,"saved":true,"playlists":true,"offline_playlists":true,
                "subscriptions":true,"settings":true,"eq_presets":true,"filters":true,"secrets":true},
 "health_reports":false}
```

(`playlists` covers both `playlists` and `playlist_entries`.)

Events to the UI via `platform::ui_event(kind, text)` → `MediaBridge.event`:

| kind | text | When |
| --- | --- | --- |
| `sync_status` | SyncStatus JSON | state, connection, key state or pending count changed (≤ 1 per second) |
| `history_updated` (existing) | "" | remote history/positions applied |
| `offline_playlists_changed` (existing) | "" | remote offline playlists applied |
| `account_updated` (existing) | "" | a synced YouTube session was installed |
| `library_changed` | `saved`\|`playlists`\|`follows`\|`filters` | remote change applied |
| `config_changed` | "" | remote settings or EQ presets applied |
| `sync_key_request` | `{"id","device_id","device_name","fingerprint"}` | `key_request` event from another device |
| `sync_key_state` | `ok`\|`missing`\|`mismatch`\|`waiting` | key state changed |
| `presence_changed` | "" | presence event/gone |
| `handoff_received` | `{"from_name","media","position_ms","playing","queue"}` | handoff to this device |
| `remote_command` | `{"from_name","command","position_ms"?,"volume"?}` | remote to this device |
| `friends_changed` | "" | friend_request/added/removed |
| `friend_activity` | "" | friend_activity |
| `inbox_item` | `{"id","from":{"username"}}` | inbox_item |
| `shared_playlist_changed` | playlist id | after the shared pull applied |
| `jam_invite` | `{"room_id","title","from":{"username"}}` | jam_invite |
| `jam_changed` | room id | jam_changed (UI refetches) |
| `jam_state` | JamPlayback JSON | jam_state (UI runs `jam_target`) |
| `jam_ended` | `{"room_id","reason"}` | jam_ended |

Android-side kind (sent by `RustActivity`, not Rust): `sync_pair_link` with the full URI.

### 8.3 Local stores and collection mapping

New `MobileState` fields (all `#[serde(default)]`, old `library.json` keeps loading):

```rust
pub resume: HashMap<String /*media key*/, ResumeEntry>,   // replaces `positions`
pub plays: Vec<PlayEvent>,                                // history events, newest last, ≤ 5000
pub playlists: Vec<LocalPlaylist>,
pub follows: Vec<Follow>,
pub filters: Vec<Filter>,
pub sync_meta: SyncMeta,                                  // per-register clocks of local items
pub struct ResumeEntry { media: Media /*slim*/, ms: u64, duration_ms: Option<u64>, updated_at: i64, finished: bool }
pub struct PlayEvent { id: Uuid, url, title, creator, kind: String, thumbnail: String, at: i64, seconds: f64, duration: Option<f64>, finished: bool }
pub struct LocalPlaylist { id: Uuid, title: String, created_at: i64, shared_id: Option<Uuid>, role: Option<String>, entries: Vec<LocalEntry> }
pub struct LocalEntry { id: Uuid, position: String, media: Media }
pub struct Follow { channel_id: String, title: String, thumbnail: String, at: i64 }
pub struct Filter { id: Uuid, kind: String, value: Option<String> }
pub struct SyncMeta { clocks: HashMap<String /*"collection\u{1f}key"*/, i64> }
```

Migration on load: every old `positions` entry (raw URL → ms) becomes a `resume` entry
under `PlaybackHistory::key` of a minimal Media `{url, id: derived}` with
`updated_at = 0`, title filled from `recent`/`saved` when the URL matches; then
`positions` is emptied and saved. `position` op keeps its request/response (`url`,
`milliseconds`) and accepts optional `duration_ms` and `media`; lookups go through the
key. Entries with `updated_at = 0` and an empty title are never pushed.

| Collection | Local source | Key | Value pushed | Applying a remote change |
| --- | --- | --- | --- | --- |
| `positions` | `resume` | media key | core `Position {media (slim), seconds = ms/1000, duration, updated_at = ms, finished}` | replace entry if remote `updated_at` ≥ local; tombstone removes |
| `history` | `plays` | `PlayEvent.id` | §6.3 fields | insert/replace by id; newest-at events also update `recent` (dedupe by URL, keep order by `at`, cap 100); tombstone removes the play |
| `saved` | `saved` | media key | slim Media | add (append) / remove by key |
| `playlists` + `playlist_entries` | `playlists` where `shared_id` is None | playlist id / `pl/entry` | `{title, created_at}` / `{position, media}` | upsert header/entry; header tombstone removes the playlist; entries whose header is missing are kept hidden until it arrives |
| `offline_playlists` | `offline_playlists` | id | `OfflinePlaylist` | upsert/remove definition (downloads are never started by sync) |
| `subscriptions` | `follows` | channel id | `{channel_id, title, thumbnail?}` | upsert/remove |
| `settings` | `Config` portable fields (server's `PORTABLE_SETTINGS`; `download` as the `PORTABLE_DOWNLOAD_FIELDS` subset) | field name | field value | merge into current config (for `download` only those fields) and `commit_config`; emit `config_changed` |
| `eq_presets` | `Config.equalizer_custom` | preset name | `{gains}` | upsert/remove in `equalizer_custom`, `commit_config` |
| `filters` | `filters` | filter id | `{kind, value?}` | upsert/remove |
| `secrets` | provider session | `youtube` (+ `keycheck`) | envelope §7.6.2 of the `vidcue_providers::auth::Session` JSON | decrypt; install through the same code path as `import_cookies` (begin an account change, `account::save`, rebuild `Browser`, clear resolver), emit `account_updated`; tombstone does nothing (a local disconnect is device-local) |

`collection_watches` is not used on Android (no local store) and is never pulled into
local state. Local clocks: hooks set `sync_meta.clocks[...] = now` when they change a
register so the first-sync union (§7.4) can compare.

### 8.4 Change hooks in existing ops (client-core edits `lib.rs`)

After the existing `mutate` succeeds, enqueue ops (when signed in and the collection is
enabled; otherwise only record the clock):

- `save_media` → `saved` add/tombstone.
- `history_record` → new `PlayEvent {at: now, seconds: 0}` (remember its id as the
  current play for that URL in memory) → `history` op.
- `position` with `milliseconds` → update `resume` → `positions` op (coalesced by the
  outbox, so frequent saves send one op per sync run); also update the current
  `PlayEvent.seconds = max(seconds, ms/1000)`, `finished` when `ms == 0` after the end or
  `ms ≥ duration - 5 s` → `history` op.
- `history_clear` → tombstones for every `plays` id and every `resume` key.
- `offline_playlist_*` → `offline_playlists` op(s).
- `save_settings` → diff old/new portable fields and `equalizer_custom` → `settings` and
  `eq_presets` ops.
- `import_cookie_file`, `import_cookies` success → `secrets/youtube` op (when key state
  is `ok`). `disconnect` → no op.

### 8.5 Filters in browse results

Applied by the engine to the items of `browse` results for Home, Subscriptions feed,
Music home, Search and `Related`, and to `up_next`/`prepare_context` candidates. Never
applied to Saved, Library, local playlists, offline, history, a channel's own page
(`Collection` of a channel id) or an explicitly opened playlist. A media item is hidden
when any filter matches:
`channel` — value equals `media.channel_id`, or equals `media.creator` case-insensitively;
`keyword` — value (trimmed, case-insensitive) is a substring of `media.title`;
`shorts` — URL path starts with `/shorts/`;
`watched` — `resume[key].finished`;
`live` — `media.live`.
The same function is in `vidcue_sync::filters` (pure) so the server can mirror it later.

### 8.6 Secret storage and background work

- `SecretStore` implementation in `sync_bridge.rs`: Android → files
  `<root>/sync/session.enc` and `<root>/sync/e2e.enc`, each the result of
  `platform::account_cipher("sealAccount", value)` and opened with `"openAccount"` (the
  existing Keystore AES-GCM alias `ytui.account.v1`; separate files, so a YouTube
  disconnect never touches them). Non-Android (tests) → plaintext files under a temp dir.
- State/outbox/health files in `<root>/sync/` (not secret: they hold library data,
  like `library.json`). Never in the cache dir.
- Engine task on `runtime()`: started by `Engine::new` when a session exists; owns the
  ws connection while foreground (`sync_foreground`), debounced pushes, retries.
- `SyncWorker` (Android, §9) calls `sync_now` every 15 min with `NetworkType.CONNECTED`.

---

## 9. B3 Android UI (owner android-ui)

### 9.1 Design rules

Minimal text, icon-first, artwork-led, existing components only (`Panel`,
`SettingGroup`, `ValueRow`, `SettingToggle`, `Glyph`, `MediaTokens`, artwork cards from
`MediaDesign.kt`/`DesignSystem.kt`). No new text-heavy screens; labels are one or two
words; numbers big; avatars as monogram circles (no new image loading for people).
Every interactive element gets a `testTag`. New glyphs (if any: `Devices`, `People`,
`Jam`, `Inbox`, `Key`) are appended to the end of `enum class Glyph` in `MediaDesign.kt`
with their drawing branch — the only `MediaDesign.kt` edit allowed.

### 9.2 New files (all in `apps/android/app/src/main/java/dev/ytui/android/`)

| File | Contents | Engine ops |
| --- | --- | --- |
| `SyncController.kt` | `internal class SyncController(vm, scope)`: StateFlows (status, devices, friends, inbox, jam room, presence, recs, playlists, follows, filters), RPC wrappers, `onEvent(event: JSONObject): Boolean` handling every kind in §8.2 (returns true when handled), `onPlayer(snapshot)` → throttled `presence_update`, WorkManager scheduling | all |
| `SyncPanel.kt` | Settings category "Vidcue": signed out → server, username, password, code fields + Sign in + "Pair" (shows how: open the camera on another device's QR). Signed in → status dot + last sync, Sync now, collection chips (toggle each), health toggle, Devices list (revoke), Key status row (opens KeyRelayUi), Security (recovery codes, password, authenticator), Admin row if `is_admin`, Sign out | `sync_status`, `sync_sign_in`, `sync_now`, `sync_settings`, `sync_devices`, `sync_revoke`, `sync_sign_out`, `account_*` |
| `PairingUi.kt` | "Show pairing QR" sheet (password prompt → QR + countdown); incoming `sync_pair_link` confirm sheet (server host, key present icon, Join / Cancel) | `sync_pair_start`, `sync_pair_inspect`, `sync_pair_claim` |
| `QrCode.kt` | `@Composable fun QrCode(rows: List<String>, modifier)` drawing modules on a white Canvas (keeps a light quiet zone in dark theme); pure `parseRows()` for tests | — |
| `KeyRelayUi.kt` | Requester: big 12-digit fingerprint + waiting state; granter: incoming request sheet with the same fingerprint, Approve / Deny | `key_fingerprint`, `key_request_start`, `key_requests`, `key_grant` |
| `SocialUi.kt` | Friends sheet: monogram row, add by username, incoming requests (accept/decline), activity cards (artwork + monogram), share-activity selector (off / now playing / history as three icon chips); Inbox sheet (artwork list, tap plays, swipe deletes); `SendToFriendSheet(media)` (friend chips + optional note) | `friends`, `friend_*`, `friends_activity`, `social_settings`, `inbox*` |
| `PlaylistsUi.kt` | Local playlists list (cover mosaics), playlist page (entries with existing `Reorder.kt` drag), create/rename/delete, `AddToPlaylistSheet(media)`, share with friends (member chips) | `playlists`, `playlist_*`, `shared_playlist_*` |
| `JamUi.kt` | Jam sheet: current artwork large, play/pause/next/seek (guarded by permission), queue with vote hearts and counts, add from search via card menu, member monograms with online dots, host settings (guests can control toggle, end), invite friends sheet, invite banner | `jam_*` |
| `JamController.kt` | Runs while in a room: on `jam_state` and every 2 s calls `jam_target` with the player's media key/position/playing and applies `load`/`play`/`pause`/`seek` through the existing view-model playback calls | `jam_target`, `jam_control` (`ended` when the player finishes the current Jam item) |
| `DevicesUi.kt` | Player overflow "Devices" sheet: other devices with now-playing artwork; tap → Handoff (send current media+position+queue) or Remote pad (play/pause, ±seek, next/prev, volume slider) | `presence_devices`, `handoff_send`, `remote_send` |
| `RemoteControl.kt` | Executes `handoff_received` (start media at position, replace queue after confirm-free start) and `remote_command` (maps to existing VM player functions) | — |
| `StatsUi.kt` | Stats page (period chips week/month/year, big totals, top creators as artwork row, by-hour bars) and Wrapped (full-bleed artwork pages, swipe) | `stats`, `stats_wrapped`, `health_summary` |
| `RecsUi.kt` | "For you" shelf composable for Home (hidden when disabled/empty), opt-in toggle row for SyncPanel | `recs`, `recs_settings`, `recs_refresh` |
| `FiltersUi.kt` | Filters list (kind icon + value, remove), add keyword, toggles for shorts/watched/live; `hideChannel(media)` helper for the card menu | `filters`, `filter_add`, `filter_remove` |
| `FollowUi.kt` | Follow toggle button for channel headers; followed-channels monogram/avatar row | `follows`, `follow_toggle` |
| `AdminUi.kt` | Users list, invite (password, admin toggle, hours → link + QR + share), reset link (password + code → QR), devices per user with revoke, disable/enable, health aggregate | `admin_*` |
| `SyncWorker.kt` | `class SyncWorker(ctx, params) : Worker` → waits for `EngineStartup` readiness (max 20 s) then `NativeBridge.request({"op":"sync_now"})`; `SyncWorker.schedule(ctx)` = unique periodic work `vidcue-sync`, 15 min, `NetworkType.CONNECTED`, `KEEP`; `SyncWorker.cancel(ctx)` on sign-out | `sync_now` |
| `SyncLifecycle.java` | static `foreground(boolean)` posting `sync_foreground` off the main thread | `sync_foreground` |

### 9.3 Wiring into hotspots (the only edits outside new files)

| File | Edit |
| --- | --- |
| `ComposeHost.kt` | (1) `SettingsPanel` group "This device"→ add a first group `"Vidcue" to listOf(Triple("Vidcue", Glyph.Account, "Sync, friends, Jam"))` and `if (shown == "Vidcue") SyncPanel(vm)`; (2) card menu: add *Send to friend*, *Add to playlist*, *Add to Jam* (only in a room), *Hide channel* entries calling the sheets/helpers above; (3) Home feed: one `RecsShelf(vm)` call above the first shelf; (4) player overflow: *Devices* and *Jam* entries; (5) channel page header: `FollowButton(...)`; (6) host the sheets (`PairConfirmSheet`, `KeyRequestSheet`, `JamInviteBanner`) at the root next to the other sheets |
| `RustRepository.kt` | `val sync = SyncController(this, viewModelScope)` field; first statement inside the `events … forEach { event ->` loop: `if (sync.onEvent(event)) return@forEach`; in the snapshot loop after `player` is updated: `sync.onPlayer(player.value)` |
| `RustActivity.java` | `onNewIntent`: before the `shared` handling, if `ACTION_VIEW` and `data.scheme == "vidcue"` and `data.host == "pair"` → `MediaBridge.event("sync_pair_link", data.toString()); return;`; `onStart`/`onStop` → `SyncLifecycle.foreground(true/false)` |
| `AndroidManifest.xml` | on `RustActivity`: `<intent-filter><action android:name="android.intent.action.VIEW"/><category android:name="android.intent.category.DEFAULT"/><category android:name="android.intent.category.BROWSABLE"/><data android:scheme="vidcue" android:host="pair"/></intent-filter>`. No camera permission, no camera library: the system camera app opens the QR. |
| `PlaybackService.java` | the two `"op","position"` calls add `.put("duration_ms", player.getDuration())` when > 0; after the first frame/audio: `health_record {kind:"start", category:"ok", duration_ms: first_frame_ms}`; on a fatal player error: `health_record {kind:"error", error_text: <exception message>}` (the engine classifies and discards the text) |

Security in UI: the pairing confirm sheet always shows the server host and requires a
tap; QR and recovery codes use `FLAG_SECURE` on their window while shown; passwords and
codes are never kept in saved state (`remember`, not `rememberSaveable`).

---

## 10. Test plans

Every agent: deterministic and offline (no real network, cookies or accounts);
`cargo fmt --all -- --check`, `cargo clippy -p <crate> --all-targets -j 2 -- -D warnings`
for the crates you touched; never weaken an existing test; report results in your change
note with levels (fixture/unit only — none of this is live).

**server-b4** (`VIDCUE_TEST_DATABASE_URL=…/<own db> cargo test -p vidcue-server -j 2`):
unit tests for message parsing/validation, token buckets, queue ordering, playback
math; DB tests in `tests/social_realtime.rs`: friend request/accept/remove (+ cascades),
404 for strangers, activity respecting opt-in levels, inbox only between friends + cap +
throttle, shared playlist create/member/push/pull/LWW/duplicate/full/non-member 404,
Jam create/invite/join/full/leave/host-end, permissions (`guests_can_control`), votes
reorder, `ended` idempotence, lifetime expiry via the test clock; a real WebSocket test
(`axum::serve` on `127.0.0.1:0` + `tokio-tungstenite`): `hello.features`, ping/pong,
presence fan-out and `presence_gone`, handoff/remote only to the target device and
`not_found` for another user's device, rate limiting, binary frame close, `jam_state`
delivery to members only.

**server-b5**: unit tests for every enum (unknown values rejected), report validation
edges, HTML escaping, CSP hash matches the inline style, CSRF compare; MusicBrainz/ListenBrainz parsers on fixtures; recs build with fake
`PublicMedia`/`MusicSimilarity` (dedupe, filters, history exclusion, budget exhaustion,
`VIDCUE_RECS=off` → 404); DB tests in `tests/personal.rs`: health post/summary/admin
aggregate/daily cap/retention, stats period + wrapped against seeded history (time-zone
bucketing, `seconds < 5` ignored, empty year), admin web login (throttle, non-admin same
failure, cookie attributes, idle expiry), CSRF/Origin refusal, disable/enable (login and
existing tokens refused, last-admin protection), admin JSON endpoints.

**client-core** (`cargo test -p vidcue-sync -p vidcue-mobile -j 2`): crypto vectors
(envelope round trip, AAD binding, wrong key fails, sealed box round trip, tampered
byte fails, all-zero shared secret refused, fingerprint fixed vector), pairing URI
parse table (valid, missing fields, bad scheme/host, http non-local, bad key length),
fractional property test, outbox coalescing/backoff/401 handling, sync engine against a
**mock HTTP server** (a local axum or hyper server in the test, not the real server)
covering first-sync union, conflict skip, cursor persistence, shared playlist pull;
ws client reconnect/backoff and event parsing against a local tungstenite server;
clock offset and drift decisions; `health::classify` table; `vidcue-mobile`: state
migration from an old `library.json` fixture (positions → resume), hooks enqueue the
expected ops, filters in browse results, settings apply keeps device-local fields,
secrets apply/push gated by key state. `cargo tree -p vidcue-sync -i aws-lc-rs` empty.
Do not run Gradle.

**android-ui** (the only agent allowed to run Gradle, one build at a time):
`apps/android/gradlew -p apps/android :app:testDebugUnitTest` with new pure-logic tests
(`QrCodeTest` row parsing, `SyncControllerTest` event routing with fake JSON,
`JamControllerTest` action application, pairing link recognition). If client-core's
engine is not merged yet, UI code must still compile against the op names in §8.1
(RPC is JSON). `tools/android/build.sh` and emulator checks only after client-core is
merged; otherwise state "not run" in the change note.

---

## 11. Deferred (explicitly out of this pass)

- Server depending on `vidcue-sync` for shared types (a `protocol`-only feature) —
  after both sides land.
- Routing the main player's own controls through the Jam room (in this pass the Jam
  sheet has the room controls; the 2 s drift loop re-aligns local deviations).
- Playback-rate nudging for small drift, member "buffering/ready" barriers, Jam chat.
- Hiding disabled users from friends lists and Jam rooms (needs B4 + B5 merged).
- `collection_watches` on Android; desktop client.
- Recommendations from YouTube account data, ML ranking, more than the three sources.
- Email/self-service reset, push notifications (not wanted anyway).
- Collaborative playlist roles beyond owner/member; per-entry author display.
- Health reports for downloads from `OfflineService` (kind exists; only `resolve`,
  `start`, `error` are wired now).
- Web admin pagination (fine below a few hundred users).
- Remote revocation of the synced YouTube session ("sign out everywhere").
