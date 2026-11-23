# Android UI for account sync, friends, Jam and personal services (B3)

- **Date:** 2026-11-23
- **PR:** none yet
- **Milestone:** B3 Android UI (docs/backend/PLAN.md), contract docs/backend/PROTOCOL.md §8.1, §8.2 and §9
- **Platforms:** Android (`apps/android`)
- **Status:** fixture/unit verified. The UI compiles and its pure logic is tested; it has not run against the engine (client-core's ops were built in parallel), on an emulator or on a device.

## What it does
- **Vidcue settings category.** A new first group in "You" opens the Vidcue page (`SyncPanel.kt`).
  - Signed out: fields for server, username, password and authenticator/recovery code, plus a "Pair" sheet. The sheet says how to scan another device's QR with the system camera, or takes a pasted `vidcue://pair` link.
  - Signed in: a monogram with a status dot and the last-sync age, a sync-now button, and a hub of icon tiles (Friends, Inbox with an unread badge, Jam, Devices, Stats, Admin). Tiles show only when the server reports the feature.
  - Also signed in: the recommendations opt-in, one chip per synced collection, the playback-health toggle, the device list with remove, key status, "Pair a device", "Password & codes" and sign-out. Sign-out can also remove this device and clear the synced library.
  - Playlists, Following and Filters are local and stay available without an account.
- **Pairing** (`PairingUi.kt`, `QrCode.kt`).
  - "Pair a device" asks for the password, then shows the engine's QR (drawn from its module rows, black on white in both themes) with a countdown. The link text, which carries the key fragment, is never shown.
  - `vidcue://pair` links open the app through a new manifest intent filter. `RustActivity` turns them into a `sync_pair_link` event and removes the link from its retained intent.
  - A confirm sheet always shows the server host and a key mark, and needs a tap on Join. Join is disabled while another account is signed in.
- **Key relay** (`KeyRelayUi.kt`).
  - Requester side: a large 12-digit fingerprint while it waits; the sheet closes when `sync_key_state` reports `ok`.
  - Granter side: a root-hosted sheet with the device name and the same fingerprint, plus Approve and Deny. Approve is only enabled for a well-formed fingerprint.
- **Account security.** View the number of recovery codes left and generate new ones (shown once, never stored). Change the password, optionally signing out other devices. Change the authenticator (QR plus a grouped secret, then confirm a new code).
- **Social** (`SocialUi.kt`).
  - Friends: three icon buttons set the share-activity level (private, now playing, history). The sheet also has a row of friend monograms with remove, add by username, incoming requests (accept/decline), outgoing requests (cancel), and activity cards (artwork with the friend's monogram, and recent plays).
  - Inbox: tap plays the item and marks it read; swipe or the cross deletes.
  - "Send to friend" from any card: pick a friend, add an optional note.
- **Playlists** (`PlaylistsUi.kt`).
  - The list shows covers and a shared mark.
  - A playlist page has play, shuffle, drag to reorder (through `Reorder.kt`; `after_entry_id` comes from `PlaylistMoves`), swipe to remove, rename, delete or leave, and sharing with friends as monogram chips.
  - "Add to playlist" from any card adds to an existing playlist or creates a new one.
- **Devices, handoff and remote** (`DevicesUi.kt`, `RemoteControl.kt`).
  - The Devices sheet lists the account's other online devices with their now-playing artwork. A remote pad has previous, ±10 s seek (estimated from the last presence update), play/pause, next and a volume slider. "Hand off" sends the current item, its position and up to 25 queued items, then pauses the local player.
  - An incoming handoff starts the media at the given position through the existing recovery-start path and then replaces the queue. Remote commands map onto the view model's existing player commands.
  - Presence updates are throttled in the UI (`PresenceThrottle`) and sent only for online media.
- **Jam** (`JamUi.kt`, `JamController.kt`).
  - Start a room (guests-can-control toggle), or accept or decline invites from the sheet or a top banner.
  - The room shows member monograms with online dots, the current artwork large, seek, play/pause and next (only for the host, or for guests when allowed), and the queue with vote arrows and counts. Anyone can play an item; the host or the person who added it can remove it.
  - The host can rename the room, change the guest permission and remove members. "Add to Jam" appears in the card menu while in a room.
  - `JamController` calls `jam_target` on every `jam_state` and every 2 s, and applies load, play, pause or seek through the existing view-model calls. It does not repeat a load of the same item for 12 s and does nothing while a start is in flight. It sends `ended` once per item when the local player reaches the end.
- **Personal.**
  - `StatsUi.kt`: Stats page (week, month and year chips; hours, plays and items as big numbers; top creators and items as artwork; plays by hour as bars; playback health when available), and a swipeable yearly review with full-bleed artwork pages.
  - `RecsUi.kt`: a "For you" shelf above Home's first shelf when the user has opted in and there are items, plus the opt-in, similar-artists and refresh rows.
  - `FiltersUi.kt`: switches for Shorts, watched and live, hide-by-word, channel filters with "Show again", and "Hide channel" in the card menu (with Undo).
  - `FollowUi.kt`: a follow toggle in channel page headers and a Following sheet.
- **Admin** (`AdminUi.kt`, admins only).
  - A users list (devices, last seen, admin and disabled marks) with health totals.
  - Invites: lifetime chips, admin toggle and password, then a QR and a share action.
  - Per user: reset link (password and code, then QR and share), remove devices (one or all), and disable/enable. Each of these asks for the password.
- **Background and lifecycle.**
  - `SyncWorker` is unique periodic work (`vidcue-sync`, 15 min, `CONNECTED`, `KEEP`). It is scheduled when a session appears and cancelled on sign-out; a run without a session cancels the schedule.
  - `SyncLifecycle.foreground` posts `sync_foreground` off the main thread from `onStart`/`onStop`. The call carries the current playing state and is skipped while full-screen video covers the activity.
  - `PlaybackService` adds `duration_ms` to its position saves and records `health_record` start reports (first frame for video, first audio for music). For fatal errors it sends `error_text`, which the engine classifies and discards.
- **Security in the UI.**
  - Passwords and codes live in plain `remember` state and are cleared after use.
  - QR codes, recovery codes and the authenticator secret are shown in a dialog window with `SecureFlagPolicy.SecureOn`, and the activity gets `FLAG_SECURE` while they are visible.
  - The confirm sheet always shows the host before a claim.
  - Events with malformed JSON are dropped; pairing links over 512 characters, with whitespace, or not exactly `vidcue://pair?…` are ignored.

## Wiring into hotspots
- `ComposeHost.kt`:
  - a "Vidcue" settings group and the `SyncPanel` category;
  - `SyncCardActions` in the card menu (Send to friend, Add to playlist, Add to Jam, Hide channel);
  - `RecsShelf` as a Home grid item after "Jump back in";
  - `SyncPlayerEntries` (Devices, Jam) in the player-options sheet;
  - `SyncSheetHost`, `PairConfirmSheet`, `KeyRequestSheet` and `JamInviteBanner` at the root.
- `RustRepository.kt`:
  - the `sync` field;
  - `sync.onEvent(event)` as the first statement in the events loop;
  - `sync.onPlayer(player.value)` after the player update.
- `RustActivity.java`: the pairing link before the `shared` handling, and `onStart`/`onStop` lifecycle calls.
- `AndroidManifest.xml`: the `vidcue://pair` intent filter. No camera permission is added.
- `PlaybackService.java`: `duration_ms` on position saves and `health_record` calls.
- `MediaDesign.kt`: the glyphs `Devices`, `People`, `Jam`, `Inbox` and `Key`, appended to the enum with their drawings.
- `BrowseRows.kt`: one line that puts `FollowButton` in the channel page header (see deviations).

## Deviations from the contract
- **Channel header location.** The channel page header lives in `BrowseRows.kt` (`CollectionHeader`), not in `ComposeHost.kt`, so the `FollowButton` call is a one-line edit there.
- **Extra files.** `SyncParts.kt` holds shared sync composables and helpers: the runner, secure dialog, toggle, tiles, monogram and text helpers. `SettingToggle` in `ComposeHost.kt` is private, and new code belongs in new files.
- **Sheet hosting.** `SyncSheetHost` hosts all sync sheets from one `SyncController.sheet` state, so card-menu and player entries open sheets without more hotspot edits.
- **Vote mark.** Votes use the existing `Up` glyph as a mode button (accent ink and a dot when voted). There is no heart glyph, and the contract allows only the five listed glyphs.
- **Shared playlist members.** The `playlists` op returns no member list, so the share dialog cannot show who is already a member of an already-shared playlist. It shows members added in the current dialog only.
- **When the engine is first read.** `SyncController` reads `sync_status` and the local library the first time the root host composes (`start()`), not in the view model's constructor. This keeps engine calls out of unit tests that build `YtuiViewModel`.
- **Change note name.** This note follows the task's name (`2026-11-23-android-sync-ui.md`) instead of the contract's `2026-11-23-backend-b3-android-ui.md`.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Kotlin compile and Android unit tests | `apps/android/gradlew -p apps/android :app:compileDebugKotlin :app:testDebugUnitTest --no-daemon --max-workers=2` | BUILD SUCCESSFUL; 359 tests, 0 failures, 7 skipped (unchanged existing skips); new: `SyncControllerTest` 18, `JamControllerTest` 8, `QrCodeTest` 3 | fixture/unit |
| Rust formatting | `cargo fmt --all -- --check` | clean (no Rust changed) | — |

New tests:
- `QrCodeTest`: row parsing (square, `0`/`1` only, size limits) and whole-pixel module layout.
- `SyncControllerTest`:
  - event routing for every §8.2 kind, with existing kinds left to the view model;
  - status, sign-out clearing, and dropping malformed payloads;
  - pairing-link recognition;
  - key-request validation and fingerprint grouping;
  - inbox and Jam event handling;
  - optional fields being left out of requests;
  - the presence throttle;
  - media keys matching `PlaybackHistory::key`;
  - the remote-command mapping and remote position estimate;
  - playlist move anchors, recommendation cards, friend and filter shapes, and text helpers;
  - the worker's engine wait and the Wrapped pages.
- `JamControllerTest`: parsing `jam_target`, load/play/pause/seek decisions (grace period, busy player, local media refused, toggles only towards the room state), the end window, and guest permissions.

## Not verified and limits
- **Not run against client-core's engine.** It was not merged into this branch. Every engine call uses the op names and shapes of PROTOCOL.md §8.1, but no call has been exercised end to end.
- **No real use yet.** `tools/android/build.sh`, an emulator run and physical-device checks were not run: there was no engine with the new ops. Text entry, the deep link, `FLAG_SECURE`, WorkManager scheduling, handoff and Jam sync have not been seen working on a device.
- **Jam timing not measured.** Drift is handled by the engine's `jam_target`; the 300–500 ms tolerance has not been measured.
- **Health reports cover starts and fatal errors only.** Stalls and downloads are not reported yet; downloads are deferred in the contract.

## For the plan and parity tables
- B3 Android UI: account sign-in and pairing, key relay, account security, friends, inbox, send-to-friend, shared playlists, devices, handoff, remote, Jam, filters, follows, stats, Wrapped, recommendations, health toggle, admin and the background SyncWorker are implemented in Compose against the PROTOCOL.md §8 engine ops. Verified at fixture/unit level only; engine integration, emulator and device acceptance are open.
