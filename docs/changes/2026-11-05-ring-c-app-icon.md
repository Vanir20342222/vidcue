# Ring C app icon (M9)

- **Date:** 2026-11-05
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9 (Android) and desktop packaging
- **Platforms:** Android, desktop packaging, docs
- **Status:** implemented

## What it does
Replaces the old teal `>_` mark with the owner-selected "Ring C" logo (concept 6 of 6): a
amber-to-pink gradient "C" ring around a white play cue on `#14111A`.

- Android launcher: adaptive icon (`mipmap-anydpi-v26/ic_launcher{,_round}.xml`) with a
  background colour, gradient foreground and a monochrome layer for Android 13+ themed icons.
- Android notifications (playback, prepare, offline service, export) now use a white
  silhouette `ic_vidcue_notification` instead of the full-colour square, which status bars
  rendered as a solid block.
- `ic_ytui` keeps its name (sign-in screen references it) but now draws the new mark.
- Desktop: `packaging/vidcue.svg`; `vidcue.desktop` uses `Icon=vidcue`; the Arch recipe
  installs it to `hicolor/scalable/apps` and the release tarball includes it.
- README hero shows the new mark instead of the spinning record.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| XML well-formed | Python `xml.dom.minidom` parse of every new/changed resource and the manifest | pass | unit |
| Visual preview | Vector XML converted to SVG and rendered in Chromium with circle/squircle/square masks, themed tint, 24dp notification and hero | renders as intended | fixture |

## Not verified and limits
- No Android SDK in this environment: the APK was not built, so `aapt2` has not compiled the
  vector drawables (gradient `aapt:attr` stroke, arc path data). CI's Android build is the first real check.
- Not checked on an emulator or a physical device launcher; not checked in a Linux desktop launcher.
- The `install.sh` source installer still does not install a `.desktop` file or icon (unchanged behaviour).

## For the plan and parity tables
- Branding: Vidcue "Ring C" icon used for Android launcher (adaptive + themed), notifications,
  desktop entry and README (2026-11-05).
