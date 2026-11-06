# Reshuffle Home recommendations (M9)

- **Date:** 2026-11-06
- **Milestone:** M9
- **Platforms:** desktop / Android / shared engine
- **Status:** fixture/unit verified

## What it does

Home already bypasses the provider response cache, but YouTube can still return
the same ranked recommendations. Each fetched Home page now randomizes its cards
and moves the previous top card down if it would remain first and there are
multiple cards. Video and Music keep separate previous-top state, shared across
Browser clones and scoped to the existing account/network provider instance.
The first load also shuffles, so restarting does not always show server order.

This uses the existing UUID randomness dependency. It retains the returned videos,
metadata, shelf labels, categories, notices and continuation tokens. Search,
collections, category requests and continuation pages keep their server order.
Android's existing explicit-refresh cache invalidation still requests a new Home
page; revisiting a prepared tab can continue to use its cached page.

## Evidence

- `cargo test --workspace --locked` on latest main plus this change: 461 passed; 0 failed; 31 ignored.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- New loopback regression repeats identical responses for video and Music through
  Browser clones, verifies a different top card and unchanged membership and
  continuation, and checks search/continuation ordering.
- New empty/single-card regression passes.
- `cargo fmt --all --check` and `git diff --check`: passed.

## Not verified and limits

No live YouTube or Android/device check was run. This changes recommendation
order; it cannot guarantee YouTube supplies entirely different videos. No APK
was built or installed.


## For the plan and parity tables

M9 / shared browsing: Home recommendations shuffle on each fetched first page,
including refresh, and avoid repeating the previous top card when multiple cards
are available. Video and Music have separate order state. Offline fixtures verify
repeated-response behavior and preservation of non-Home ordering; live service
and device acceptance remain open.
