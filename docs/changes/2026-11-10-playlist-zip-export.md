# Export offline playlists as a zip (M9, #139)

- **Date:** 2026-11-10
- **PR:** not opened
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented · naming helper JVM-unit verified; no Android build or device run

## What it does
The offline playlist sheet has an "Export as zip" action. It asks where to save a `.zip`
named after the playlist, then writes every item that has a complete offline copy into it as
`001 - Title.m4a`, `002 - Title.m4a`, and so on, in playlist order. Items without a complete
copy are left out, and the status line reports how many. The export runs in the background like
single-file exports, reads only the completed offline cache, and contacts no server.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| `PlaybackPolicyTest` (incl. new `playlistZipsShareFolderNamingRules`) | `javac` + JUnit 4.13.2 on `PlaybackPolicy.java` and the test | 11 tests pass | fixture/unit verified |
| `PlaylistZip.java` type check | `javac` against stub `android.*`/`org.json` types and the real `PlaybackPolicy` | compiles | compile-only |
| Android-dependent Java syntax | `javac -proc:none` on `OfflineExport`, `PlaylistZip`, `RustActivity`, `MediaBridge` | no syntax errors | parse-only |

## Not verified and limits
- No Android SDK in the build environment: `:app:compileDebugJavaWithJavac`, Kotlin compile and
  `:app:testDebugUnitTest` were not run. `OfflineLibrary.kt`, `OfflineExport.java` and
  `RustActivity.java` are reviewed but not compiled against the Android libraries.
- The zip writer, SAF file creation, cancellation and the partial-file cleanup are not exercised
  on an emulator or device.
- Items with no complete copy leave gaps in the numbering, so the prefixes keep the playlist
  positions. The status line reports how many were left out.
- The zip is written with DEFLATE. Media is already compressed, so it saves little space.
- A later rename or reorder does not change an exported zip. Export again to get a new one.

## For the plan and parity tables
- Playlists row: "Offline playlists export as one zip of numbered files in playlist order, with
  items that lack a complete offline copy skipped. Verified on JVM tests and parse checks only;
  device and live acceptance pending."
