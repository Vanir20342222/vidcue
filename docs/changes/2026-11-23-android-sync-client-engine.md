# Sync client library and Android engine integration (B3)

- **Date:** 2026-11-23
- **PR:** none yet
- **Milestone:** B3 (docs/backend/PLAN.md), contract docs/backend/PROTOCOL.md §7–§8 (client side of §2–§6); M9 Android engine
- **Platforms:** shared engine (`crates/vidcue-sync`, `crates/vidcue-mobile`)
- **Status:** fixture/unit verified; real-server integration verified locally against the B1–B5 `vidcue-server` binary (127.0.0.1, synthetic accounts). No device, emulator or deployed-server run.

## What it does
- **New crate `crates/vidcue-sync`.** Pure Rust with no Android code, so a desktop app can reuse it. It contains:
  - the HTTP API client (`api`) for every B1–B5 endpoint the app uses;
  - the persisted, coalescing outbox (`outbox`);
  - the push/pull engine (`engine`): first-sync union, conflict skip, cursor, shared playlists and health flush;
  - the end-to-end key (`crypto`): the XChaCha20-Poly1305 secrets envelope, the X25519/HKDF key-relay sealed box and the 12-digit fingerprint;
  - the pairing URI (`pairing`), fractional positions (`fractional`), the Jam clock and drift rule (`clock`);
  - the WebSocket client (`ws`, rustls with `ring` only), serde types for every message (`protocol`), local error classification (`health`), the pure content filters (`filters`) and QR rows (`qr`).
- **`vidcue-mobile` integration.** Every engine op of §8.1 exists: sync and account, key relay, pairing, social, shared playlists, presence, handoff, remote, Jam, health, stats, recommendations, admin, and local playlists, follows and filters.
  - New local stores: `resume`, `plays`, `playlists`, `follows`, `filters` and `sync_meta`, all `#[serde(default)]`.
  - Old URL-keyed `positions` migrate to media-keyed `resume` entries on load. The `position` op is unchanged for callers and resolves through the media key.
  - Existing ops queue sync writes after a successful local change (§8.4).
  - Pulled changes apply to the library, config, EQ presets and the YouTube session. Device-local settings (proxy, folders, hooks, endpoints) are never pushed or applied.
  - Filters hide items in Home, Subscriptions, search, music sections, related items and related up-next.
  - A background loop on the engine runtime runs debounced pushes and retries with backoff. It also owns the WebSocket while the app is visible.
  - UI events are sent through `MediaBridge.event` (`platform::ui_event`).
- **Secrets on Android.** The device token and account key live in `<files>/sync/session.enc` and `e2e.enc`, sealed by the existing `ytui.account.v1` Keystore key through `sealAccount`/`openAccount`, separate from `account.enc`. State, outbox and health files are in `<files>/sync/`. Nothing new goes into the cache directory.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Library unit tests | `cargo test -p vidcue-sync -j 2` (lib) | 30 passed. Covered: envelope round trip, AAD binding, wrong key and tampering; sealed box round trip, every tampered byte, wrong requester, truncation, all-zero shared secret; fingerprint fixed vectors (checked independently with Python `hashlib`); pairing URI table (valid, missing/duplicate/unknown fields, bad scheme/host/path, http on a non-local host, bad code, bad key length); server origin rules; 10 000 random fractional inserts keep order and ≤ 64 chars; spread/rebalance; outbox coalescing, 20 000 cap (history dropped first, then a refusal), backoff 2 s → 5 min with ±20 % jitter, 429 fixed wait, file shape; clock offset (smallest RTT, discard > 1 s), expected position, drift decisions with the 3 s seek backoff; `classify` table; health buffer bound; contract JSON literals for every event and object; QR quiet zone | unit |
| Library engine and WebSocket tests | `cargo test -p vidcue-sync --test engine` against an in-process axum mock (`crates/vidcue-sync/tests/support/mock.rs`) | 8 passed. Covered: first-sync union (remote newer wins, local-only pushed, key created and key check pushed); pending local write beats an older remote change; 503 backoff then `offline`; cursor persisted and reused after reopening; rejected count; 401 → `signed_out`/`revoked` with the outbox kept for the same account and dropped for another; key relay end to end (missing → waiting → ok, pending secret unlocked); QR pairing key; wrong pairing key → `mismatch` with no secret queued; shared playlist push/pull with its own cursor and no repeat pull; health batches of 100 only when enabled; disabling a collection drops its queue; WebSocket hello/features, `rid` replies, typed and unknown events, reconnect after a drop, 401 on upgrade → signed out | fixture |
| Engine integration tests | `cargo test -p vidcue-mobile --test sync` (same mock, engines in temp dirs) | 8 passed. Covered: migration from an old `library.json` fixture (positions → resume, titles from saved media, other URL forms resume); every hook queues the expected register (saved, history with listened seconds, positions, playlists and moved entries, subscriptions, filters, settings without proxy or folders, offline playlists, tombstones after unsave and `history_clear`); remote apply into every store, including an entry before its playlist header, a playlist tombstone, an `equalizer` choice before its preset, settings that keep device-local fields, and ignored `collection_watches`; YouTube session gated by key state, unlocked by the relay; QR pairing inspect/claim; presence, handoff and remote events; local files refused for handoff; self-revocation keeps the library; health text classified and never stored; shared playlists between an owner and a member (share, add, leave); Jam target (load, seek with lead and backoff, play, leaving) | fixture |
| Real server, end to end | `cargo build -p vidcue-server`, then `VIDCUE_SERVER_BIN=target/debug/vidcue-server VIDCUE_TEST_DATABASE_URL=postgres://…/vidcue_test_b3 cargo test -p vidcue-mobile --test server_e2e` (run 4 times) | Passed. Steps: real invite, signup and TOTP; two password sign-ins using recovery codes; every collection pushed with **0 rejected** values; second device receives the library; key relay; QR pairing third device; presence and handoff over the real WebSocket; Jam room with clock target and pause; health reports and summary; stats; friends; admin users and invite with local QR rows; revocation → `revoked`. Skipped (passes) when the variables are unset | local integration |
| Existing tests | `cargo test -p vidcue-mobile` | 57 unit tests (52 existing plus 5 new; 2 ignored as before) and the 17 unchanged `tests/engine.rs` tests pass | unit |
| Lint | `cargo fmt --all -- --check`; `cargo clippy -p vidcue-sync -p vidcue-mobile --all-targets -j 2 -- -D warnings` | clean | — |
| Android target | `cargo +1.99.0 check -p vidcue-mobile --target aarch64-linux-android` with the NDK 27.2 linker env of `tools/android/build.sh` | clean | build |
| TLS gate | `cargo tree -p vidcue-sync -i aws-lc-rs`, same for `vidcue-mobile` | no match (`ring` only) | build |

## Choices where the contract left room, and deviations
- **Change note name.** This note uses the name the task gave, not `2026-11-23-backend-b3-sync-client.md`.
- **`fractional::between` returns `Option<String>`.** `None` means no position of at most 64 characters exists (for example `"a"`/`"a0"` or equal positions), and the caller respaces with `fractional::spread`. Open ends step by one digit instead of halving, so 2000 appends stay at most 40 characters. Results are short but not always the shortest possible.
- **`ApiError` is an enum:** `Unauthorized`, `Http { status, message }` or `Network`.
- **`LocalStores`** has two extra default methods for shared playlists: `shared_playlists` (create, rename or drop local copies) and `apply_shared`. For `secrets`, `apply` and `snapshot` carry plaintext JSON; the engine encrypts and decrypts. The key check never reaches the app.
- **Background start.** The task needs a shared engine, so it starts from `initialize()` through `Engine::start_sync(self: &Arc<Self>)`, right after `Engine::new`. It is not started inside `Engine::new`. The WebSocket opens on `sync_foreground {visible: true}` and right after sign-in or claim, not at engine start, so a background WorkManager start opens no socket.
- **Debounce.** Ordinary edits push after 2 s. Pushes that carry only `positions`/`history` wait 30 s, because Android saves the position every 5 s. An urgent trigger (WebSocket change, `sync_now`, sign-in) runs at once.
- **First-sync union** queues only local registers with a strictly newer clock. On a tie the remote copy wins, so values just applied from the server are not echoed back.
- **Re-enabling a collection** resets the cursor and repeats the first-sync union, so it catches up.
- **`state.json` extra fields:** `user_id` (a different account discards the outbox), `signed_out_reason`, the last `keycheck`, encrypted `pending_secrets` (secrets that arrived without a key), `resend_secrets`, `server_features`, `last_clock`, `rejected_count`, `health_reports`.
- **A 401** deletes the `session` secret but keeps the outbox and state. The status then reads `signed_out`/`revoked`.
- **Key states.** A key from a pairing link is treated as `ok` until the first pull contradicts it; then the state becomes `mismatch` and queued secrets are dropped. When no key check exists after the first pull, this device creates the key.
- **EQ presets are ordered before settings,** both in diffs and in the first union. A pulled `equalizer` that names a preset not yet received is kept in `sync_meta.pending_equalizer` and applied when the preset arrives.
- **`sync_sign_out {forget_data: true}`** clears only the new B3 stores: playlists, follows, filters, play events, resume entries and sync metadata. Saved media, offline playlists, settings, the queue and downloads stay.
- **`SyncStatus`** adds a `notice` field (outbox full, relay failure, last error). It never contains tokens.
- **Health.**
  - `resolve` outcomes are recorded only for non-speculative resolves.
  - `client_version` is the engine crate version.
  - `health_record` without `category` or `error_text` records `ok`.
- **Filters** are not applied to `Continue` pages, because the engine cannot tell which feed a continuation token belongs to. Music sections arrive as `Category` requests and are filtered.
- **`recs` items** are flattened into Media objects that carry `reason` and `source`.
- **Account ops** rename fields to the contract: `account_recovery_regenerate` returns `codes`, and `account_totp_start` returns `secret`/`otpauth_uri`/`qr`.
- **Pairing URIs** with unknown query parameters are rejected.
- **`jam_target` with nothing current** returns `pause` while the local player is playing.
- **Host builds** (tests only) store sync secrets as plaintext `.txt` files with mode 0600. Android always seals them.

## Not verified and limits
- **No Gradle, emulator or device run.** The Kotlin UI belongs to android-ui. Android-only code (`platform::ui_event` and Keystore sealing through the existing `sealAccount`/`openAccount`) was compiled for `aarch64-linux-android` but never executed.
- **No Java change was needed.** `ui_event` calls the existing package-private static `MediaBridge.event(String, String)`.
- **For android-ui:**
  - Call `sync_foreground` from `RustActivity.onStart`/`onStop`, including `playing`.
  - Cancel `SyncWorker` after `sync_sign_out`.
  - `PlaybackService` should pass `duration_ms` (and optionally `media`) to `position`.
- **No deployed server, real network, TLS endpoint or WebSocket through a reverse proxy was used.** The end-to-end run used plain `http://127.0.0.1`.
- **Renaming a shared playlist** sends the server PATCH in the background; a failure is not reported.
- **Events are not always delivered while offline:**
  - Jam `jam_state` events only arrive while the socket is up; the UI refetches on `jam_changed`.
  - Friend events are delivered only while connected.
- **Old history** (the `recent` list without timestamps) is not converted into play events. Stats start from the first play synced after this change.
- **Shared playlist entries** use `playlist_*` ops with the same caps as personal playlists (5000 locally). The server caps them at 2000 and reports anything beyond that as rejected.

## For the plan and parity tables
- B3 client library and engine (2026-11-23): `vidcue-sync` crate and `vidcue-mobile` integration implemented. Verified with 51 new offline tests (unit tests plus integration tests against an in-process mock) and in a local end-to-end run against the real `vidcue-server` with PostgreSQL: all collections accepted, key relay, QR pairing, WebSocket handoff, Jam clock, health and admin. Android target compiles. No device, emulator, Kotlin UI or deployed-server acceptance yet.
