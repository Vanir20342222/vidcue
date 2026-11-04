# Android video controller and captions menu (M9)

- **Date:** 2026-11-04
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented; compiled, unit tested and rendered locally (Robolectric). No emulator or device run.

## What it does
The owner (2026-11-04, with a screenshot of the inline player) did not like the video player UI and
said the caption options were "an endless list".

- **Controller.** The screenshot was the 16:9 inline player on a phone, about 412 x 232 dp, carrying
  chrome sized for fullscreen. The redesign:
  - Draws the transports on flat dark discs without the glass edge. The 48 dp controls draw a 44 dp
    disc, and play is 56 dp inline and 64 dp in fullscreen (it was 72 dp).
  - Slims the top bar (52 dp) and bottom bar (44 dp), sets the timecode at 12 sp with a dimmed
    duration, and lowers the scrims.
  - Centres the seek amount inside the rewind and forward rings in the app's face.
  - Keeps every ContrastUiTest contract: a 48 dp target, a local backing that reaches the sampled
    point, 3:1 for white glyphs on white video (`videoChromeDisc`, checked by `MediaDesignTest`)
    and the unchanged timeline accent.
- **Captions button.** Media3's subtitle button opened a list of every caption track. ytui's own
  button now switches captions on or off. It is shown only when the media has captions, and its
  icon follows the state. Switching on without a choice picks the creator's captions in a language
  the device uses, then the automatic captions in the spoken language, then a translation
  (`Captions.best`).
- **Caption tracks.** YouTube offers the creator's captions, automatic captions in the spoken
  language and machine translations into well over a hundred languages. Media3 had attached the
  first 40 in catalogue order (alphabetical translations), with labels like
  "en · automatic English". They are now ordered by use and named readably: "English",
  "English (auto-generated)", "German (auto-translated)" (`Captions.configurations`). A translation
  into the spoken language, which duplicates the generated track, is dropped.
- **Playback options.**
  - The root rows show current values: Captions ("English · auto", "Off"), Audio (only when there
    is a choice) and Sleep timer.
  - "Audio & subtitles" is split. **Captions** offers Off, the creator's and auto-generated tracks
    and translations into the device's languages, each with a selected mark and a one-line source.
    The remaining machine translations fold into "More languages" with a search field.
  - **Audio** offers the extractor languages and in-stream audio tracks. The old raw list of video
    resolutions is gone; Video quality already covers it.
  - The audio language rows use the app's choice rows instead of stock text buttons with a "✓".
- **Status JSON.** Each track gains `type`, `language`, `name`, `height`, `selected` and, for text
  tracks, `kind`. The status gains `captions_on`, and there are new `captions_on` and
  `captions_off` commands. The existing `label` field is unchanged.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Kotlin and Java compile | `gradlew :app:compileDebugKotlin :app:compileDebugAndroidTestKotlin` | pass | build |
| Unit tests | `gradlew :app:testDebugUnitTest` (incl. `MediaDesignTest` disc contrast on the new token) | pass | unit |
| Lint | `gradlew :app:lintDebug` | pass | build |
| Visual review | Local Robolectric harness. Controller: a still Media3 `SimpleBasePlayer` over a synthetic frame with dark UI and a bright window, phone inline and landscape fullscreen. Menus: options root, Captions, More languages, search ("por"), Audio, with a realistic 40-track caption set | reviewed | fixture render |

Test change: `CatalogUiTest` opens the Captions row and expects the Off choice or the no-captions
message, instead of "Audio & subtitles" and "Toggle captions".

## Not verified and limits
- No emulator or device. Real video under the controller, touch feedback, caption rendering and
  track switching on real YouTube media were not run. The emulator suite (including
  ContrastUiTest) now runs nightly.
- Caption kinds are inferred from the engine's catalogue labels ("· automatic", "-orig"). If
  YouTube or the extractor changes that format, tracks fall back to the "embedded" kind and the
  plain language name.
- Media3 still attaches at most 40 caption tracks. Languages beyond that are not offered, but the
  order now makes sure those are rarely used translations.

## For the plan and parity tables
- Android: quieter native video controller sized for inline and fullscreen; captions on/off
  button; caption tracks ordered and named by use; Captions page with Off, suggested languages and
  a searchable list of machine translations; separate Audio page; option rows show current
  values (M9; fixture-rendered, not device verified).
