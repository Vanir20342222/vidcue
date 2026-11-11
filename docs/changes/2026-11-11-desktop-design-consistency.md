# Desktop design consistency pass (M9)

- **Date:** 2026-11-11
- **PR:** #160
- **Milestone:** M9
- **Platforms:** desktop
- **Status:** fixture/unit verified (headless renders of all 28 synthetic scenes)

## What it does
Audits the Rune desktop app against its own design tokens and fixes the places that broke them.
Section and group labels are sentence case in one `ty::LABEL` style instead of tracked capitals
("LIBRARY", "THEME", "RELATED", "IMPORT A PALETTE"). Tags (Live, Alpha, New, Video, Audio,
Offline, Missing) share one `badge` style. Eyebrows above titles ("FEATURED", "COLLECTION",
"PLAYLIST", "OFFLINE PLAYLIST") are removed or folded into the meta line ("Collection by
Kitchen Atlas · 12 items"). Corner radii follow a five-step scale (`sz::R_XS`…`R_PANEL`) where
there were about 30 ad-hoc values: rows 12, popovers 14, menu items and fields 8, heroes 18.

Specific defects fixed:
- Downloads: audio jobs' square covers shifted the title column left of video jobs; all
  artwork now sits in the same 100 px slot.
- Player settings menu showed raw ids for Equalizer and Colour grade ("off" next to "Off").
- Movies hero: Play was 44 px tall beside 38 px buttons, and the "Another pick" icon used the
  theme's text colour over artwork (dark-on-dark in light themes); it now uses the overlay button.
- Collection and Movies Play buttons match the height of the buttons beside them.
- Onboarding tagline no longer trails off ("— beautifully").

A unit test (`theme::tests::screens_use_the_type_scale`) fails when a screen sets its own font
size or tracking, or capitalises a label in code.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Desktop tests | `cargo test -p vidcue-desktop` | 89 passed, 3 ignored (live, opt-in) | unit/fixture |
| Guard test catches a violation | added `.tracking(0.8)` to the wordmark, reran | fails naming `sidebar.rs`; reverted | unit |
| Lint | `cargo clippy -p vidcue-desktop --all-targets -- -D warnings`, `cargo fmt --all -- --check` | clean | — |
| Visual | `vidcue-desktop --snap … --scene <all 28> --size 1440x900`, before and after compared | labels, alignment, button heights as described | headless render |

## Not verified and limits
- No interactive session on Omarchy/Ghostty or Windows; headless scenes are synthetic.
- Spacing literals (`gap`/`padding`) were not migrated to a scale; only radii and type were.

## For the plan and parity tables
- Desktop: one type scale and radius scale enforced by a unit test; sentence-case labels and
  shared badges across screens (2026-11-11).
