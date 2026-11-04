# Theme overhaul and Nothing-styled themes (M9, shared themes)

- **Date:** 2026-11-04
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9 (shared palettes, Android appearance); desktop theme picker
- **Platforms:** shared engine, desktop, Android
- **Status:** fixture/unit verified (desktop PTY captures; Android Kotlin not compiled locally)

## What it does
The built-in theme list now has 15 entries instead of 13, ordered in picker sections:
`system`, the ytui pair, the new Nothing pair, then editor palettes that also ship as
Omarchy themes (the current desktop target), then `terminal`.

- **Added:** `nothing-dark`, `nothing-light`, `everforest`, `kanagawa`, `matte-black`,
  `flexoki-light`. Everforest, Kanagawa, Matte Black and Flexoki Light follow the
  current Omarchy `colors.toml` values (basecamp/omarchy, master). Some roles were adjusted
  to meet the contrast gate: Kanagawa muted and error, the Matte Black success green,
  and the Flexoki 600-level status colours.
- **Retired:** `dracula`, `rose-pine`, `solarized-dark` and `gruvbox-light`. None of them
  ships with Omarchy, and each duplicated a kept palette. The `gruvbox-light` palette was
  the only warm light theme and is replaced by `flexoki-light`. Migration is lossless.
  `Config::theme` uses a serde hook, so every TOML/JSON load path (desktop `rust.toml`,
  profiles, Android `config.json`, `save_settings`) maps a retired name to its replacement:
  dracula and rose-pine → catppuccin-mocha, solarized-dark → nord, gruvbox-light →
  flexoki-light. `builtin()` resolves them too.
- **Android:** the picker no longer lists `terminal`, which on Android rendered as a copy of
  ytui Dark. A saved `terminal` value still resolves.

### Nothing style research
Sources: Nothing's community posts on Nothing OS design and fonts, and third-party
design-system write-ups of nothing.tech. Nothing has published no official spec, so the hex
values are approximations.
- Colour: a monochrome base. The dark mode uses true black (OLED), white type and grey
  secondary text. One red signal colour (≈ `#D71921`) is used sparingly, for example on
  widgets and the Glyph record dot. Nothing's website is purely monochrome, with ink
  `#000000`, canvas `#FFFFFF` and muted `#585A5A`.
- Type: the Ndot dot-matrix face gives the brand its character and is reserved for display
  use. Nothing OS 5 moved everyday text to Geist and uses Geist Mono in its Playground
  experiences. Earlier systems used NType 82 for headlines and Lettera Mono for uppercase
  labels.
- Materials: flat surfaces with hairline separation and no shadows.

### Implementation
- Palettes: `nothing-dark` is `#000000` / `#141414` / `#FFFFFF` / `#999999`, with accent
  `#D71921`. `nothing-light` is `#F2F2F2` page, `#FFFFFF` cards, `#000000` ink, `#585A5A`
  muted and the same red accent.
- `Palette::dot_matrix()` is true only for the built-in Nothing palettes. Imported palettes
  never opt in.
- Desktop (a terminal cannot change its font): with a Nothing theme, download progress
  meters are drawn as dot-matrix rows (`●●●···`) instead of solid bars.
- Android: Nothing themes switch Material typography. Doto (OFL, round dots via `ROND=100`)
  replaces Ndot for display and headline styles only. Geist is used for titles and body
  text, and Geist Mono for labels. Sizes stay on the shared scale, so layouts do not shift.
  The sign-in activity and fullscreen options use the same type. ytui bundles no Nothing
  proprietary fonts or logos.
- Capture tool: adds `--themes-only` and stops `write_manifest` from walking into
  `docs/screenshots/android`, which has its own manifest. Before this fix, every `--*-only`
  refresh failed with "Not WQHD".

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Palette contrast (text/muted/selection ≥ 4.5, accent/status ≥ 3) for all 15 | `cargo test -p ytui-services --lib theme::` | pass | unit |
| Retired names migrate in TOML and JSON; dot-matrix flag | `cargo test -p ytui-core theme` | pass | unit |
| Android engine lists no `terminal`; previews every listed theme | `cargo test -p ytui-mobile palettes_preview_every_built_in_theme` (in workspace run) | pass | unit |
| Dot-matrix meter | `cargo test -p ytui meter_switches_to_dot_matrix` (in workspace run) | pass | unit |
| Desktop render of the six new themes | `capture_ui --themes-only`; reviewed `docs/screenshots/themes/nothing-{dark,light}.png` | rendered | PTY fixture |
| Format, lint, all tests | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` | pass (0 failures) | unit |
| Android JVM tests for Nothing typography selection | `ThemeLabelTest` | not run locally (no Android SDK in the container); CI `:app:testDebugUnitTest` | — |

Measured contrast ratios against the background (text / muted / accent):
Nothing Dark 21.0 / 7.4 / 4.1; Nothing Light 18.8 / 6.2 / 4.6; Everforest 7.4 / 5.1 / 6.2;
Kanagawa 11.3 / 6.7 / 5.9; Matte Black 10.1 / 5.4 / 7.4; Flexoki Light 18.6 / 5.0 / 6.4.

## Not verified and limits
- The Kotlin changes were not compiled here. Android CI must pass compile, unit tests and lint.
- No emulator or physical-device review yet of Doto headlines, including at 2× font scale.
  Doto is an open stand-in; it is not Nothing's Ndot.
- Desktop screenshots for the unchanged themes were kept from their earlier capture. Only
  the six new themes were captured in this change.
- The Nothing hex values are research approximations, not an official Nothing specification.

## For the plan and parity tables
- RUST_REWRITE_PLAN.md (theme list sentence near "Rose Pine, and a restrained ytui
  dark/light pair"): "Built-in themes: system, ytui dark/light, Nothing dark/light,
  Tokyo Night, Catppuccin Mocha/Latte, Everforest, Gruvbox, Kanagawa, Nord, Matte Black,
  Flexoki Light and terminal. Dracula, Rose Pine, Solarized Dark and Gruvbox Light are
  retired and migrate automatically."
- Android PARITY: "Theme picker offers every shared palette except desktop-only
  `terminal`. Nothing themes add dot-matrix display type (Doto) with Geist/Geist Mono."
