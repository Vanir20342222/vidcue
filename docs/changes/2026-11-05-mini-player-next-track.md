# M9: Mini-player action follows playback mode

- **Date:** 2026-11-05
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented

## What it does

The Android mini-player keeps its X (stop and close) button for video playback.
Audio playback, including music and songs, shows the existing Next track icon
instead and sends the existing `next` command. The action uses `PlayerState.video`,
matching the expanded player's playback-mode distinction, so audio-only playback
of a video catalog item also receives Next track. Play/pause remains available.

## Evidence

`git diff --check` passed. The existing Next glyph and playback
command are reused. Android compilation and device checks were not run: this
workspace has no configured Android SDK. Visual and actual queue-advancement
checks on a device remain pending.

## Not verified and limits

No emulator, physical-device or live playback qualification was performed locally.

## For the plan and parity tables

M9 Android mini-player: video retains stop/close; audio playback shows Next track.
Implemented; Android CI and device verification remain pending at authoring time.
