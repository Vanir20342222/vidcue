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
  `ACTION_VIEW` with the folder's plain document URI and the directory MIME type,
  granting read access. A missing file manager or stale folder shows an error event.
- Follow-up (2026-11-11): every download of a playlist now packs into one folder.
  `StorageLocations.childFolder` reuses the playlist's folder instead of creating
  a new one per file, which made providers add `Name (1)`, `Name (2)`, ... folders.
  Removing a download deletes the folder only once it is empty; before, the delete
  relied on providers refusing to delete a non-empty folder, which they do not.
- Follow-up (2026-11-11): an automatic folder save of a download outside any
  playlist no longer fails on a missing playlist title.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Compile, unit tests, lint | `gradlew :app:compileDebugAndroidTestKotlin :app:testDebugUnitTest :app:lintDebug` | Pass (2026-11-11) | Local |
| One folder per playlist, delete only when empty | `PlaylistFolderTest` (Robolectric, in-memory document provider) | Pass | Unit |
| Open folder on device | Not run | Not run | - |

## Not verified and limits

- Not run on a device. Whether the file manager opens a folder from
  `ACTION_VIEW` depends on the device's file manager; the Files app handles it
  on Android 10 and later, older versions may report that none can open it.
- Playlists saved before the folder fix can still have several `Name (n)`
  folders on disk; new saves reuse the folder named exactly after the playlist.

## For the plan and parity tables
- Offline playlists: "Open playlist folder" in the playlist ⋮ menu opens the
  folder that playlist downloads were packed into (Android, implemented, not
  device verified).
