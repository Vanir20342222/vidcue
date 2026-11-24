# 2026-11-24 — Review fixes for B4/B5, the sync client and the Android sync UI

Milestones: B4, B5 (server), B3 client (`vidcue-sync`, `vidcue-mobile`), M9 Android UI.
Fixes from the review of everything after 0313dcd on `feat/backend-b1-sync`.

## Server
- `/admin` cookie sessions end with a password change, an authenticator change, an
  account reset and "Sign out all devices" (`admin_web::end_sessions`), not only when the
  account is disabled.
- `/admin/login` uses the `/api/login` throttle keys (`login-ip:`, `login:{user}`,
  `login:{user}:{ip}`, `factor:{user}`): one budget for both entry points, and the
  account reset clears it.
- Jam: a guest without control can only send `ended` near the item's end; votes, queue
  removals, settings, invites and declines are throttled (`JAM_EDIT`, `JAM_INVITE`);
  open invitees are not re-notified; `jam_ping` writes (expiry, keep-alive) only when due.
- `friend_remove` locks the affected shared playlists (sorted) before deleting member rows,
  the same order as playlist deletion and member removal (no deadlock).

## Sync client
- Pairing links: nothing syncs and no socket opens until the user confirms the account and
  server (`sync_pair_confirm`); the `secrets` collection is off after a link sign-in until
  switched on. The confirm sheet shows the full origin; `http` local servers in links are
  accepted only by development builds.
- First sync takes the local registers before pulling and keeps (and pushes) local values
  strictly newer than the remote ones. Clock-0 registers go out with clock 1; untouched
  default settings are not pushed.
- A secret that does not open with the current key stays pending and is retried after a
  key is adopted.
- A `shared_playlist_changed` event before the first listing creates the local copy instead
  of skipping entries; an owner's other devices merge the entries re-pushed when sharing
  stopped. Renaming a shared playlist reports a server failure instead of reverting later.
- Jam playback is only replaced by an equal or newer version (socket events, HTTP replies,
  refetches); `jam_target` returns the room `rate`. The clock estimate only improves within a
  sampling round and adopts the round's best sample at its end.
- Presence: re-sent after every `hello` (`presence_reset` resets the UI throttle); minimum gap
  2.5 s. `sync_foreground` without `visible` updates the play state while hidden.

## Android
- Sync events and the Jam/presence player hooks keep running while the UI is stopped (when
  events are queued, audio plays or a Jam is open); queued handoffs and remote commands
  older than 10 s are dropped.
- PlaybackService reports play-state changes while hidden; FullscreenActivity reports
  hidden/visible.
- Pairing confirm and key request sheets show during onboarding; a second sheet names the
  account before the first sync, with the YouTube sign-in off by default.
- Jam: local pause/play is forwarded when this member may steer the room, otherwise a notice
  explains (with Leave for another video); the room rate is applied without saving it.
- Stats/Wrapped handle system Back; owner "Delete" of a shared playlist deletes it; the
  share dialog starts from the server member list; "Added" only after success; admin
  disable/enable updates the row; SyncWorker keeps its schedule on a failed status call;
  sync status colours follow the palette; the status dot and remote volume have TalkBack
  labels.

## Not changed
- Key relay grant authentication (the requester cannot verify who granted the key): needs a
  confirmation-code flow on both devices; tracked as an open item. Link sign-ins now start
  with the YouTube sign-in off, which covers the relay after a link pairing.

## Checks
See the commit for the commands run (fmt, clippy, workspace tests with the test database,
Android x86_64 build).
