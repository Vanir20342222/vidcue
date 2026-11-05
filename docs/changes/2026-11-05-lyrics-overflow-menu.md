# Lyrics panel settings move into a ⋮ menu (M9)

- **Date:** 2026-11-05
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented (not compiled in this environment)

## What it does
The lyrics panel now shows only the song title, artist and the lyrics. The lyrics source label, the
"Follow playing line" switch (now "Stop auto-scroll" / "Auto-scroll with the song"), "Edit or find
lyrics" and "Remove saved lyrics" moved into a ⋮ menu at the top right of the panel header. While
the editor is open a "Hide editor" row stays above it. The "No lyrics found" state is unchanged.
`Panel` gained an optional `actions` slot in its header for this.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Compile | `apps/android/gradlew -p apps/android :app:compileDebugKotlin` | Not run: no Android SDK in the agent container | none (pending CI) |

## Not verified and limits
- Not compiled, not checked on an emulator or device. No instrumentation test covers the new menu yet;
  its tags are `lyrics-menu`, `lyrics-source`, `lyrics-follow` and `lyrics-editor`.

## For the plan and parity tables
- Android lyrics: panel options (source, auto-scroll, editor, remove saved) live in a header ⋮ menu.
