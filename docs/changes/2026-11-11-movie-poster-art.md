# Sharp movie posters and backdrops on Android (M9)

- **Date:** 2026-11-11
- **PR:** pending
- **Milestone:** M9
- **Platforms:** Android, shared mobile engine
- **Status:** Rust unit verified; app Kotlin compiled; Android unit tests and device check pending (no JDK 21 on this host)

## What it fixes
On the Android Movies (alpha) page, many covers were blank or blurry:

- The 39 Cinecat titles had no artwork at all and drew generated gradient art.
- The open films used `archive.org/services/img/<item>`, which serves a 180×124 landscape
  thumbnail. It was cropped into a 2:3 card about 350 px wide, a 3–4× upscale of a fragment.
- The engine fits art into a square box sized from the frame **width**, so even a sharp 2:3
  poster was decoded at two thirds of the width it is drawn at.

## What changed
- Every film and Cinecat title now carries a TMDB poster (2:3) and backdrop (16:9) from TMDB's
  public, keyless image CDN (`image.tmdb.org`). Paths were taken from each title's TMDB page in
  English and every size used (posters w342/w500/w780, backdrops w300/w780/w1280) was checked to
  serve on 2026-11-11. Stream Lab keeps its generated art.
- `movieArt` picks the backdrop for wide frames (tablet hero, Continue watching) and the poster
  otherwise, requests the smallest TMDB size that covers the frame, and sizes the square decode box
  from the cropped image's longer side.
- The artwork cache gains an opt-in `large` tier (1024/1280 px) used only by movie art, so a
  full-width hero is not upscaled from a 768 px decode. Feed covers keep the 768 px cap. The native
  decode box cap (`MAX_THUMBNAIL_PIXELS`) and `ImageCache::load_sized` height cap rise to 1280 to
  allow it; existing callers ask for the same sizes as before.
- TMDB 1091, labelled "Nosferatu" in the CLI's pool, is *The Thing* (1982), and that is what
  the sources API resolves. The Android label now says so, so it matches its poster.

## Evidence
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean.
- `cargo test -p vidcue-mobile --locked`: 51 + 17 pass. `cargo test -p vidcue-providers --lib images`: 4 pass.
- `gradlew :app:compileDebugKotlin`: compiles.
- New unit tests in `MovieCatalogTest`: every film/Cinecat title has a TMDB poster and backdrop;
  frame-to-art selection for card, phone hero, tablet hero, Continue watching and the detail sheet.

## Not verified
- Android unit tests and lint were not run locally (host only has Java 27; AGP's jlink transform
  needs JDK 21). CI runs them.
- Not checked on an emulator or physical device.
- `images::sized_tests::concurrent_requests_share_one_fetch…` fails locally in a full
  `vidcue-providers` run (fetch count 2, expected 1) on clean `main` too; it passes alone.

## For the plan and parity tables
- Android Movies (alpha): film and Cinecat posters/backdrops load at display resolution from TMDB's image CDN.
- The CLI pool (`tools/cinecat`) still labels TMDB 1091 "Nosferatu"; it is *The Thing*.
