# Android "screening room" UI redesign (M9)

- **Date:** 2026-11-03
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented; compiled and rendered locally (Robolectric, host-built engine). No emulator or device run.

## What it does
The owner asked (2026-11-03) for every panel to be redesigned without the "Android feel",
with smooth animations and a better experience. The app kept its YouTube-style layout but was
assembled from stock Material 3 parts: Roboto, ripples, filter chips, the navigation bar and
rail, outlined text fields with floating labels, Material switches and radio buttons, dropdown
menus and an all-caps `CANCEL` dialog. This change replaces those parts with one design
language, so the interface recedes and the artwork carries the colour.

- **Type.** Bricolage Grotesque (display: page names, what is playing, sheet titles) and
  Instrument Sans (everything else), bundled as variable fonts under the SIL OFL with their
  licences in `assets/licenses`. One scale lives in `YtuiType` (`DesignSystem.kt`).
- **Feedback.** Material ripples are off app-wide. `SoftIndication` draws a quick wash of the
  current ink, and a visible ring for keyboard focus. Buttons, chips, icons and navigation
  cells spring slightly when pressed. Every animation still goes through `Motion`, so
  Reduced motion and the animation style/speed settings apply.
- **Shell.** Each tab's name is the page title, set in the display face, and it rises in when
  you change tabs. Navigation is a floating dock (or a vertical dock on tablets) with a
  highlight that slides and stretches between destinations. The two-row fallback for large
  fonts and the measured-label logic are kept. The account button shows a dot when a session
  is connected.
- **Mini player.** A floating card above the dock takes on the cover's colour, with an ink
  play disc, a morphing play/pause glyph and a progress line along its lower edge. All of its
  gestures are unchanged.
- **Browse.** New search field: the edge brightens on focus, and the arrow fills once there
  is text. Chips cross-fade. Feeds that are loading show shimmer cards shaped like the result
  instead of a spinner, and pull to refresh has its own disc. A pulsing "Live" badge replaces
  `LIVE`. The card that is playing shows animated bars and an accent title. Channel avatars
  get a colour derived from the name.
- **Player.** Music borrows its cover's colour for the whole screen (`ambientGlow`). The
  rounded, shadowed cover eases back while paused. The 72dp play disc and larger
  previous/next keys sit on a seek bar that thickens while dragged and shows the remaining
  time. Queue, Lyrics, Save, Sleep and More are an evenly spaced captioned row below the
  transport. The video watch page keeps its layout with the new pills. The tablet side panel
  is tinted by the artwork too.
- **Sheets and dialogs.** Sheets have a slim handle and a darker scrim. Titles use the display
  face, and the close control is a raised disc. Untitled action sheets no longer spend a
  header row. Dialogs (theme, remove, delete, About) are app cards that settle in with a short
  scale. The theme list uses a check-in-ring mark and readable names ("Catppuccin mocha").
- **Settings and account.** Settings are grouped into cards (Look and feel, Listening,
  Connection, This device), and each row has an icon tile. The app now draws its own switches
  and choice marks. Inputs keep their label inside the field. The account panel has an
  identity badge, one primary action and grouped import and privacy cards. Downloads, queue,
  history, lyrics (lines fade as they pass), onboarding and the sign-in activity use the same
  components.

All palettes still come from the shared Rust themes. Test tags, accessible names and button
labels are unchanged. The one exception is the theme dialog's `CANCEL`, now `Cancel`; the
matching `CatalogUiTest` lookup was updated.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Kotlin compile (app + instrumentation) | `gradlew :app:compileDebugKotlin :app:compileDebugAndroidTestKotlin` | pass | build |
| Unit tests | `gradlew :app:testDebugUnitTest` | 80 tests, 0 failures | unit |
| Lint | `gradlew :app:lintDebug` | pass | build |
| Visual review (also at 2x font scale) | Local-only Robolectric harness that launches `RustActivity` with the real shared engine built for the host, synthetic covers and fixture feeds; screens captured: feeds, music, radio, downloads (offline, transfers, playlists), search, loading, empty, error, menus, card sheet, sleep, about, queue, history, browse, settings pages, theme dialog, account, mini player, music player, playback options, video watch page, tablet, onboarding (5 steps), and ytui-light, catppuccin-latte, gruvbox and rose-pine | reviewed | fixture render |

The harness itself is not committed, because it needs a host build of the engine with a test
JNI shim.

## Not verified and limits
- No emulator or physical device was available (no KVM). Instrumentation tests compile but
  were not run, including the IME, contrast, tablet and playback-layout suites.
- Robolectric cannot draw `TextureView`, so the native video frame and the Media3 controller
  overlay were not rendered. The video chrome (`PlayerChrome`, XML controller) is unchanged.
- Artwork tint is computed from the decoded cover on a background thread and cached per
  item. Its cost on low-end devices has not been measured.
- Large text was rendered at 2x font scale only (feed with two-row dock, music player).

## For the plan and parity tables
- Android UI: custom design language replaces stock Material 3 components: bundled display
  and text faces, soft press feedback, floating navigation dock, artwork-tinted mini and full
  player, skeleton loading, grouped settings, app-drawn switches, fields, menus and dialogs
  (M9, UI redesign; fixture-rendered, not device verified).
