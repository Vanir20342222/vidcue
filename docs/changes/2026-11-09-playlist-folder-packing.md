# Playlist downloads pack into folders (M9)

- **Date:** 2026-11-09
- **PR:** #<number> <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented · fixture/unit verified

## What it does

When "Save completed media to selected folders" is on, each playlist download
now packs into a subfolder named after its offline playlist inside the chosen
Music/Video folder, instead of a flat file in the folder root. Files inside the
playlist folder carry a numeric prefix (`001 - Title.m4a`) so the playlist
order survives on any device or file browser. The folder is a plain file-system
folder: copying it to another device or a PC keeps the whole playlist together.

Details:

- `OfflineExport` looks up the offline playlist containing the download id
  (`offline_playlists` op, first match) and creates/reuses the playlist folder
  via SAF `createDocument(..., MIME_TYPE_DIR)`. Reuses an existing folder with
  the same name; downloads not in a playlist keep the flat file layout.
- `StorageLocations.remember` records the playlist folder URI and shows the
  folder as `Root › Playlist` in the library rows; `remove` deletes the file and
  best-effort removes the playlist folder when it is empty.
- Naming helpers moved to `PlaybackPolicy.clean`, with `folderName` and a
  position-aware `exportName` that both sanitize and bound like export names.
- The settings toggle now describes the behavior: "Real files in your Music or
  Video folder; playlist downloads keep their own subfolder."

## Evidence

| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| `:app:compileDebugJavaWithJavac` (offline) | `./gradlew :app:compileDebugJavaWithJavac` | pass | fixture |
| `:app:testDebugUnitTest` (offline) | `./gradlew :app:testDebugUnitTest` | 251 tests pass, 0 failures, incl. 3 new `PlaybackPolicyTest` cases | fixture/unit verified |

New tests: `playlistFolderNamesFollowExportNameRules` (path-unsafe characters,
empty fallback, 100-codepoint bound) and `playlistExportsKeepTheirOrderWithANumericPrefix`
(`001 -`, `012 -`, `999 -` prefixes).

## Not verified and limits

- No emulator or physical-device run: the SAF folder creation, the folder
  reuse, the empty-folder cleanup and the `Root › Playlist` row display are not
  verified on Android yet. (Local JDK 27 could not run AGP's jlink; a JDK 21
  was installed user-level for the build.)
- The first playlist containing a download wins; a download in two offline
  playlists packs into one folder only.
- A later playlist rename or reorder does not move or rename already-exported
  files; the prefix stays at the export-time position.
- A user's own empty folder with the same name as a playlist folder would be
  removed by the empty-folder cleanup.
- Previously exported flat files (record exists) are not re-packed into folders.

## For the plan and parity tables

- Downloads row: "Completed playlist downloads export as real files grouped in
  a folder named after the offline playlist, with a numeric prefix that keeps
  the playlist order across devices; non-playlist downloads stay flat in the
  chosen Music/Video folder. Verified on JVM tests only; device and live
  acceptance pending."
