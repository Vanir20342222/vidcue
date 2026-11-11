# Hide "Finding the best stream" on desktop players (M9)

**Owner request (2026-11-11):** remove the "Finding the best stream" status from
YouTube, YouTube Music and Radio playback on the desktop app.

**Change:** `apps/desktop/src/ui/watch_controls.rs` (video) and
`apps/desktop/src/ui/music.rs` (music/radio). While a stream resolves, the spinner
still shows but the text is omitted. "Buffering…" is unchanged. The Movies
"Finding a working stream" text is out of scope and left as is.

**Evidence:** headless UI test
`ui_tests::resolving_a_stream_shows_only_the_spinner` puts the `watch` (video) and
`song` (music/radio) scenes into the resolving state. It fails on the old code for each
player separately and passes with this change. `cargo fmt`, workspace clippy and
workspace tests were run locally; the results are in the PR.

**Not verified:** no screenshot of a live desktop window. The buffering label cannot be
held in a headless test, because the mpv snapshot is refreshed on every poll; that code
path is unchanged.
