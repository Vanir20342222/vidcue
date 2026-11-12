# Android design-rules pass (M9)

- **Date:** 2026-11-12
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android
- **Status:** emulator verified (disposable API 35 AVD, 360dp and 411dp widths)

## What it does
The owner asked (2026-11-11) for a full pass over the Android UI to find issues and places
where the app's own design rules (the "screening room" language in
`2026-11-03-android-screening-room-redesign.md`, the content-first restyle and the UX overhaul)
were not applied cleanly. Each screen outside Movies was audited on an emulator and in code.
Movies browsing and catalog search belong to other open work and were not changed.

What a user notices:

- **Small phones.** On 360dp-wide phones the video controls overflowed: Next ended up with less
  than a 48dp touch target. The top and bottom shading also met in the middle of the roughly
  200dp-tall video, dimming the whole picture while the controls were shown. Below 400dp width the
  transport gaps are now tighter and the shading is shorter. Wider screens are unchanged.
- **Said once, named once.** An empty Home feed no longer shows its notice and a second, generic
  empty message under it. The notice becomes the empty-state text. Radio stations named after
  their genre ("Lofi" by "Lofi") now say "Radio station", on cards and in the mini player.
  Downloads called one action both "Save a link" and "Save from a link"; it is now "Save from a
  link" in both places. Onboarding said "Reduce motion" while Settings said "Reduced motion";
  both now say "Reduce motion".
- **Stale or misleading copy.** The playback setup step said "Choose how ytui plays", using the
  old name. The welcome line ("native, fast, yours") is now a plain description. Swipe between
  tabs no longer lists four tabs when there are five. Spelling is consistently British
  ("Colour grading", "Look and feel"). The account-needed feed error pointed at menus with "→";
  it now says what to do, and its button is "Open account" instead of a slash-joined label.
- **Settings rows line up.** Icon tiles mark categories on the Settings root only. Rows inside a
  page (theme, import palette, folders, retry, gesture tips) no longer mix icon and plain rows, so
  their text shares one left edge. Explanations that floated under a row at a different inset
  (maximum quality, audio-only saves, streaming cache, animation style) are now that row's
  subtitle. Setup guide no longer reuses the Playback icon. The cookie file row uses a folder icon
  instead of a link. Onboarding shows values in neutral ink, as Settings does, instead of the accent.
- **Sign-in window.** The one-word accent ("Continue to *YouTube*" in teal) is gone, and the title
  uses the app's type scale instead of ad hoc sizes. The window draws under the status bar in the
  page colour instead of a separate system band. Its card, field and divider use the shared shapes
  and rule.
- **Motion settings everywhere.** The sign-in, fullscreen options and onboarding animations used
  fixed durations, so Animation style and speed did not reach them. They now read the shared
  `Motion` (`ComposeHost.motionFor`), which keeps the Reduce motion behaviour.
- **One token set.** Stray corner radii and the live red now come from `MediaTokens` (`sideSheet`,
  `thumb`, `badge`, `appMark`, `cardBottom`, `live`). The update panel uses the app's progress
  bar instead of Material's, and its version heading is no longer accent-coloured. Fullscreen
  options uses the app rule and title style.

No playback, provider, Rust, networking, persistence or Movies code changed. Test tags and
accessible names are unchanged.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Compile, unit tests, lint (the PR CI job) | `gradlew :app:assembleDebug :app:compileDebugAndroidTestKotlin :app:testDebugUnitTest :app:lintDebug` (JDK 17) | pass; 291 unit tests, 0 failures; lint has no finding on a changed line | unit |
| Before/after screens | Baseline APK (main `207767f`) and this branch on the same disposable AVD (API 35 google_apis, SwiftShader): onboarding, Home, Music, Radio, Downloads, Settings pages, account sheet, sign-in, mini and full player | reviewed; changes as listed above | emulator |
| Narrow-phone transport | `ContrastUiTest` at 360dp | baseline fails "next needs a 48dp touch target"; branch passes, and the capture shows white video between the scrims | emulator |
| Touched-area suites at 411dp | `OnboardingUiTest`, `CardInformationUiTest`, `ContrastUiTest` | 6/6 pass | emulator |
| Broad UI suites at 411dp | `CatalogUiTest`, `NativeUiTest`, `TransferCardUiTest`, `StartupUiTest` plus the above | 35/39 pass. The 4 failures also fail on the baseline in this emulator (fixture video does not decode or play under SwiftShader) | emulator |

## Not verified and limits
- Full-player rendering at 1080×2400 crashed the emulator's SwiftShader renderer (multi-GB host
  allocations) on this machine, so screens were reviewed at 720×1600. The baseline and the branch
  both render the full player at that size. Full-resolution visual acceptance is unverified, and
  the crash does not show the player's blur cost is harmless.
- The four suite failures above (`musicTransitionsKeepTheDecoderAndProduceMotionEvidence`,
  `videoInformationOnHomeAndAlwaysInPlayer`, `wideLayoutKeepsTabsSettingsAndPlayerPanelsReachable`,
  `coldAndPreparedCardTapsReachAVisibleFrameWithinTwoSeconds`) fail identically on the baseline
  here. They need the nightly CI emulator to confirm.
- No physical device and no signed-in account were used. Tablet and landscape layouts were not
  re-captured.
- The sign-in return strip still stops above the gesture bar (page colour beneath it).
- Not changed, for an owner decision: the navigation dock is icon-only by design; the full player
  shows shuffle/repeat on live stations; the update panel has both a "Later" pill and the sheet's
  close control.

## For the plan and parity tables
- Android UI: design-rules pass over shell, settings, onboarding, sign-in, downloads, radio and
  video chrome: shared motion settings in every window, one shape/colour token set, consistent
  settings row anatomy and vocabulary, 48dp video transport targets and an unshaded picture
  middle on 360dp phones (M9; emulator verified, not device verified).
