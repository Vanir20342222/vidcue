# Bluetooth output button and audio output picker (M9)

- **Date:** 2026-11-05
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented (not compiled or device-checked in this session)

## What it does
The player action tray gets a Bluetooth button (between Save and Sleep). It is drawn in the
accent colour while media plays through a Bluetooth device (A2DP, LE audio or hearing aid) and in
the normal ink otherwise. Tapping it opens an "Audio output" panel listing the connected outputs
(this device's speaker, wired/USB headphones, Bluetooth devices) with the current one checked.
Picking one routes playback there with `ExoPlayer.setPreferredAudioDevice`; "Use system default"
clears the choice, and a chosen device that disconnects falls back to the system route. "More
devices" opens Android's media output switcher (`MediaRouter2.showSystemOutputSwitcher` on
Android 14+, the settings media-output panel before that, then Bluetooth settings) for paired
devices that are not connected yet.

`PlaybackService` registers an `AudioDeviceCallback` and publishes `output`
(`id`, `kind`, `name`, `automatic`, `devices`) in the player snapshot. The active route comes from
`AudioManager.getAudioDevicesForAttributes` on Android 13+; older releases use a heuristic
(Bluetooth, then wired, USB, speaker). No new permission is needed.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Glyph | Bluetooth rune rendered as SVG next to the other tray icons | Same stroke and height | visual mock |

## Not verified and limits
- No Android SDK in the session: not compiled, linted, or run on an emulator or device.
- Route detection before Android 13 is a guess and can be wrong when several accessories are
  connected. Bluetooth product names depend on the vendor/OS and fall back to "Bluetooth device".
- Not yet in the fullscreen video options or the "Playback options" sheet.
- Needs a physical check with a Bluetooth headset: tint on connect/disconnect, switching to the
  speaker and back, and the system switcher on the Galaxy Tab S9.

## For the plan and parity tables
- Android: audio output picker with Bluetooth indicator (needs physical-device check).
