# Dark video timebar keyline for mid-tone accents (M9)

- **Date:** 2026-11-05
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented; unit test added (not run locally, no Android SDK in the session)

## What it does
The Media3 video timebar's keyline was chosen with `mediaOnAccent`, the ink colour for text
drawn *on* the accent. Accents with luminance just under 0.179 therefore got a white keyline,
and the unplayed/buffered track was mixed from white, so the timeline looked very bright with a
grey outline. `mediaTimeBarBacking` now keeps the keyline black whenever the accent keeps 3:1
against black and only uses white for accents too dark for that.

Built-in palettes affected before the fix: `nothing-dark`, `nothing-light` (#d71921),
`vidcue-light` (#006f62), `catppuccin-latte` (#8839ef) and `flexoki-light` (#205ea6). All now
get the same dark keyline as the other palettes.

Other `mediaOnAccent` uses (onPrimary, switch thumb, choice check, notice badge, readable
text fallback) draw ink on top of the accent and were checked; they are correct.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Built-in accent audit | luminance/contrast script over `crates/vidcue-core/src/theme.rs` palettes | 5 palettes had a white keyline; all reach ≥3.2:1 on black | analysis |
| Unit test | `MediaDesignTest.videoTimebarKeylineStaysDarkForMidToneAccents` | added; to run in CI | unit |

## Not verified and limits
No emulator or device capture in this session. `ContrastUiTest` (instrumented) still asserts
accent/keyline contrast ≥3:1 on white video and should be rerun for the Nothing palettes.
