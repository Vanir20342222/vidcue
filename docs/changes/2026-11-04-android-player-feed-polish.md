# Android player, feed and podcast fixes from device feedback (M9)

- **Date:** 2026-11-04
- **PR:** not opened yet (owner asked to wait)
- **Milestone:** M9
- **Platforms:** Android, shared engine
- **Status:** fixture/unit verified (Rust); Kotlin syntax-checked only

## What it does

Fixes a round of Galaxy-phone feedback. YouTube Music's Podcasts feed shows its
episodes instead of an empty page. Lyrics load automatically from YouTube Music, with
LRCLIB as a fallback. Sideways swipes move between Home, Music, Radio and Downloads
instead of feed categories. The mini player and navigation float over the page. The
search bar scrolls away. Save sends the cover flying into Downloads. The music
player was reworked: no dark band at the bottom, no text jump on Next, larger covers
with the visualiser styles, and a richer visualiser. Scroll work was moved off the
main thread. The account button's "connected" dot was removed.

| Report | Change |
| --- | --- |
| Music/Radio scroll ~4 fps | The 5 Hz player snapshot is parsed and fingerprinted off the main thread. Visible-item prefetch checks (prefs, `ConnectivityManager` binder call, JNI) run on `Dispatchers.Default`. Feed grids pass cell width to covers, so a card no longer subcomposes its own `BoxWithConstraints`. Live badges in feeds keep a still dot, not one infinite animation per station. |
| Player empty space, styling, black bottom bar | `navigationBarColor` is transparent with contrast enforcement off. Full panels draw their backdrop under the system bars and pad only the content. The cover has 20 dp side margins and the actions sit in one rounded tray. |
| Swipes changed categories | `tabSwipe` on the whole content area uses the bottom-navigation order (Home, Music, Radio, Downloads). Chip and shelf rows still scroll themselves. |
| Podcasts empty | `musicMultiRowListItemRenderer` episodes are parsed (show, views, date, spoken length, video ID). `MPSP…` show pages map to their playlist. |
| Dot on profile | Removed; it indicated a connected account but read as a notification. |
| Save symbol | The "Download queued" confirmation disc is gone. The cover flies in an arc into the Downloads tab, which bounces. |
| YouTube Music lyrics | New `lyrics_auto` engine op: anonymous `next` → lyrics tab → `browse`. It uses timed lines from the Android Music client when present, plain text otherwise, then LRCLIB timed lyrics. The sheet loads them automatically and names the source. |
| Text jumps on Next | The music title is one line (marquee when long). The artist line and seek slot keep a fixed height while the next item has no duration. |
| Small covers with visualisers | Neon VU and Pulse Orbit use a square stage with covers at 66% and 56% of its width (previously 45% and 32%). |
| Visualiser | Per-band smoothing, a mirrored 64-bar spectrum ring and breathing glow (orbit), spectrum bars in the VU tank, and a slight cover swell on bass. |
| Search bar on scroll | Search and filter chips slide away while scrolling down. They return on an upward scroll, at the top, on re-selecting the tab or while searching. |
| Mini player blocks view | The dock overlays the page with a transparent surround. Grids and the Downloads list add the dock's height to their bottom padding. |

## Evidence

| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Podcast parsing | `cargo test -p ytui-providers podcast` | Passed (synthetic fixture) | unit |
| YT Music lyrics | `cargo test -p ytui-providers music_lyrics` | 5 passed: timed, plain fallback, no tab, untimed rows and ID extraction; no cookies sent | unit |
| Provider/engine suites | `cargo test -p ytui-providers -p ytui-mobile --locked` | 50 + 17 + 72 passed | unit |
| Lint | `cargo fmt --all`, `cargo clippy -p ytui-providers -p ytui-mobile --all-targets --locked -- -D warnings` | Clean | static |
| Live podcast shape | Anonymous WEB_REMIX `browse` of the Podcasts chip, parsed by a temporary local test (not committed) | Before: 0 items. After: 27 episodes with show names and lengths | live, one-off |
| Live lyrics | Temporary local test against three public songs (not committed) | Two returned YouTube Music lyrics (one credited to Musixmatch); one returned none, so the engine falls back to LRCLIB | live, one-off |
| Kotlin | Gradle's bundled `kotlin-compiler-embeddable` 2.0.21 run over the changed files without Android dependencies | No parse errors; type checks need the SDK | syntax only |

## Not verified and limits

- The Android SDK could not be installed in this session. The app and the test APK
  were **not compiled**, and no unit, instrumentation or lint task ran. Run
  `:app:compileDebugKotlin :app:compileDebugAndroidTestKotlin :app:testDebugUnitTest :app:lintDebug`
  before merging.
- Scroll smoothness is not measured. The 4 fps report came from the debuggable
  Preview build, and Compose runs much slower in debuggable builds. Frame timing must
  be compared on a device, preferably also with a non-debuggable build.
- Floating dock: instrumentation tests that assume the dock sits below the feed (not
  over it) may need their geometry expectations updated.
- YouTube can change the lyrics tab, the timed-lyrics model or the podcast renderers.
  Line timings exist only where YouTube supplies them.
- Without a device, the Save fly-in target and the visualiser were not checked on the
  phone, tablet rail or in landscape.

## For the plan and parity tables

M9 Android: the YouTube Music Podcasts feed parses episode (`musicMultiRowListItemRenderer`)
and show (`MPSP…`) items. Lyrics load automatically from YouTube Music by video ID
(anonymous, timed when available) with LRCLIB fallback. Tab swipes follow the bottom
navigation order, including Downloads. The mini player and navigation float over
content. The music player draws edge to edge, keeps fixed-height title, artist and
seek slots, and gives the visualiser styles larger covers and a spectrum ring/bars.
Rust unit-verified; Android not yet compiled or device-tested.
