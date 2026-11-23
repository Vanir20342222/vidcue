# Self-hosted backend: friends, shared playlists, presence and Jam (B4)

- **Date:** 2026-11-23
- **PR:** none yet
- **Milestone:** B4 (docs/backend/PLAN.md), contract docs/backend/PROTOCOL.md §1–§5
- **Platforms:** backend (`apps/server`)
- **Status:** fixture/unit verified (server only; not deployed, no app client yet)

## What it does
`vidcue-server` now has the B4 social and realtime features:

- **Friends.** Friendships need both sides to agree. Activity sharing is off until a user opts in (`now_playing` or `history`).
- **Inbox.** Users can send media to a friend.
- **Shared playlists.** They use the same per-entry merge as synced playlists.
- **WebSocket.** It now accepts client messages: presence, handoff and remote control between a user's own devices, and the Jam clock ping.
- **Jam rooms.** Rooms only coordinate playback. They have a queue with voting, a host setting for whether guests may control playback, and a server playback state that every device aligns to.

Migration 4 adds the tables. The server never fetches or resolves media.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Server tests | `VIDCUE_TEST_DATABASE_URL=…/vidcue_test_b4 cargo test -p vidcue-server -j 2` | 49 passed (all existing tests plus 17 B4 unit and 6 B4 database/WebSocket tests); the B4 database tests ran 3 more times without failure | fixture/unit |
| B4 unit tests | `presence`, `social`, `jam` modules | Covered: message envelope/rid parsing, presence, handoff and remote validation, token buckets, `to_device` delivery filter, activity coalescing, `jam_ping` write interval, queue ordering, playback math (expected position, re-anchoring, rate, bounds), action parsing, note/title/entry validation, fixed error messages | unit |
| Friends, activity, inbox | `tests::social_realtime::friends_activity_and_inbox`, `friend_caps` | Covered: request/accept/decline/remove, 404 for strangers and unknown users, opt-in levels (off hides, `now_playing` hides history, `history` lists 7 days of history with only six cleaned fields), inbox only between friends, slim media, note rules, the 200-item cap that evicts read items, 90-day retention, the 20/h and 60/h throttles, 50-request and 100-friend caps | fixture |
| Shared playlists | `tests::social_realtime::shared_playlists` | Covered: create (idempotent per owner), friend-only members, owner-only rename/delete, leave/remove, push LWW/duplicate/superseded/rejected, the 2000-entry cap where tombstones still apply, 404 for non-members, pull paging and cursor past title changes, unfriend cascade, the 50-playlist limit | fixture |
| Jam | `tests::social_realtime::jam_rooms_queue_votes_and_permissions`, `jam_rooms_expire_after_twelve_hours_and_count_actions` | Covered: create/invite (friends only)/join/decline/full/kick/leave/host end, one room per user, the first item starting at once, vote ordering, `guests_can_control`, idempotent `ended`, play_item/next/stop, position and rate bounds, unfriend cascade, 30-minute idle and 12-hour expiry via the test clock, maintenance deletion, the add throttle | fixture |
| WebSocket | `tests::social_realtime::websocket_presence_handoff_remote_and_jam` (`axum::serve` on `127.0.0.1:0` + tokio-tungstenite) | Covered: `hello.features`, ping/pong with rid, presence fan-out to own other devices only, snapshot, `friend_activity` to friends only, handoff/remote only on the target device, `not_found` for another user's device, local media refused, presence and message rate limits, unknown type, `jam_state`/`jam_invite`/`jam_ended` to members only, `jam_ping` ack and refusal, expiry announced, `presence_gone`, 1008 after three bad messages, 1003 on binary | fixture |
| Lint | `cargo fmt --all -- --check`, `cargo clippy -p vidcue-server --all-targets -j 2 -- -D warnings` | clean | — |

## Choices where the contract left room
- **`friend_activity` coalescing.** Changes inside the 10 s window are coalesced into one event at the end of the window, carrying the state at that time. They are not dropped. Turning sharing on or off sends one event to friends at once.
- **`presence_snapshot`** lists every online device of the user. A device that has not sent presence yet appears with an idle state (`media: null`, `can: []`, `at`/`server_at` = connect time), so it can still be a handoff target.
- **Presence state fields** `duration_ms`, `volume` and `at` are optional. When `at` is missing, the server uses its own time. The stored form always has every field (with `null` where unknown). Wrong JSON types → `bad_message`; values out of range → `invalid`; a handoff queue over 25 → `too_large`.
- **Disconnect.** When a device disconnects, its now-playing is cleared for friends through the same coalesced `friend_activity` path.
- **`presence::on_message` signature.** It takes a per-socket `Connection` (which holds the token buckets and the error count, and removes presence when dropped) instead of `(&state, &device, &text)`. This is internal to `api.rs`.
- **Shared playlist pull.** When `more` is false, `next` advances to the playlist `seq`. Otherwise a title or member change, which has no entry, would make clients pull again forever. All seq assignments for one playlist hold its advisory lock, and the pull reads one snapshot, so no entry is skipped.
- **`POST /api/shared-playlists` with an id owned by someone else** returns 409 `This playlist id is taken`.
- **Jam actions with nothing current.** `play` with nothing current starts the top queue item (409 `The queue is empty`). `pause`, `seek` and `rate` with nothing current return 409 `Nothing is playing`.
- **Jam invites.** Invites over the 20-open limit are refused with 409. An invite to an existing invitee is refreshed and listed again.
- **Votes.** A guest who leaves or is removed loses their votes in that room. Their queue items stay.
- **Room expiry events.** These are sent when a member next touches the room (HTTP or `jam_ping`). Rooms ended by the hourly `maintenance` produce no events, because it has no access to the hub. A member's `jam_ping` within 60 s normally triggers the event first.
- **Lock order to avoid deadlocks.** Friendship rows are share-locked before room or playlist locks in invite and add-member, the same order as unfriending. Membership changes take a per-user advisory lock and lock all involved rooms in id order.
- **`Hub::device_online`** is the "online lookup made `pub(crate)`". `SocketSlot` and `reserve_socket` keep their visibility.

## Not verified and limits
- Nothing in this note is deployed or tested against the live server (the backend container), a real network or an app client. All checks are fixture or unit level.
- Disabled users (B5 `users.disabled_at`) still show up in friend lists and Jam rooms; deferred by the contract until B4 and B5 are merged.
- Presence and activity windows are per server process and in memory. After a restart, `now_playing` is `null` until devices report again.
- A guest (not only the host) may invite their own friends who are not the host's friends, as the contract allows.
- The WebSocket test checks that events do *not* arrive by waiting 100–300 ms. It is reliable locally, but it is timing-based.

## For the plan and parity tables
- B4 server (2026-11-23): friends with opt-in activity, inbox, shared playlists, WebSocket presence/handoff/remote, and Jam rooms with clock ping, queue votes and host permissions are implemented and verified at fixture level (49 server tests). Not yet deployed. The client is B3.
