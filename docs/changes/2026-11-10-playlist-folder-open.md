# Open a playlist's folder from its offline playlist menu (M9)

- **Date:** 2026-11-10
- **PR:** #<number> <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented

## What it does

Each offline playlist row now has a ⋮ menu with "Open playlist folder". It
asks the system to open the folder that the playlist's downloads were packed
into (see the playlist folder packing note) in the device's file manager. If no
download in the playlist has a folder yet, it shows a message instead. This
resolves GitHub issue #138.

Details:

- `StorageLocations.playlistFolder` finds the first member of the playlist, in
  playlist order, whose offline file record has a `playlistFolder` URI.
- `MediaBridge` handles the `playlist_folder` effect off the UI thread and sends
  `ACTION_VIEW` with that document URI and the directory MIME type, granting
  read access. A missing file manager or stale folder shows an error event.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Kotlin and Java compile | Android SDK not available in this session | Not run | - |
| Open folder on device | Not run | Not run | - |

## Not verified and limits

- Not compiled: no Android SDK in the session. Compile `:app:compileDebugKotlin`
  before merging.
- Not run on a device. Whether the file manager opens a folder from
  `ACTION_VIEW` depends on the device's file manager.
- `OfflineExport` creates a new SAF folder for each file of a playlist download
  instead of reusing one, so a playlist can have several folders. The item
  opens the first one that has a file record, which may hold only some of the
  playlist's files. Fixing the reuse is a follow-up.

## For the plan and parity tables
- Offline playlists: "Open playlist folder" in the playlist ⋮ menu opens the
  folder that playlist downloads were packed into (Android, implemented, not
  device verified).
