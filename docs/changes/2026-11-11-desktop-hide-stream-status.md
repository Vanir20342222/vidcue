# Hide "Finding the best stream" on desktop players (M9)

**Owner request (2026-11-11):** remove the "Finding the best stream" status from
YouTube, YouTube Music and Radio playback on the desktop app.

**Change:** `apps/desktop/src/ui/watch_controls.rs` (video) and
`apps/desktop/src/ui/music.rs` (music/radio). While a stream resolves, the spinner
still shows but the text is omitted. "Buffering…" is unchanged. The Movies
"Finding a working stream" text is out of scope and left as is.

**Evidence:** `cargo check -p vidcue-desktop --offline` finished with exit 0 on the
pre-rebase branch. Rustfmt check on the two edited files passed.

**Not verified:** no runtime or screenshot check of the desktop player; no
`cargo clippy` or `cargo test` run for this change yet.
