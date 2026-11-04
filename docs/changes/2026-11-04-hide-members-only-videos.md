# Hide members-only videos and explain access failures (M9/M2/M5)

- **Date:** 2026-11-04
- **Milestone:** M9 / M2 / M5
- **Platforms:** Android, desktop and shared engine
- **Status:** fixture/unit verified
- **Request:** fix the reported no-format error, then do not show members-only videos

## What it does

The shared parser removes videos and songs with recognized channel-membership
badges before returning or caching a page. This applies to Home, search,
subscriptions, channel/playlist results, related videos and their continuations,
regardless of whether an account is connected. No additional setting or
per-video network request is introduced.

Both classic renderer badges (`BADGE_STYLE_TYPE_MEMBERS_ONLY`) and modern
lockup metadata/thumbnail badges (`BADGE_MEMBERS_ONLY` and the thumbnail badge
variant) are recognized, with English/German membership badge text as a
fallback. Titles, descriptions, creator names, owner verification badges,
Premium badges and membership menu actions do not imply membership-only
content. Playlists/channels and local saved/offline data are preserved.

A positive membership marker also hides unmarked duplicates on the same page,
including when the marked card is explicitly unplayable or lacks a title.
Category chips, shelf labels and continuation tokens survive filtering. If a
page contains only hidden members-only rows, the browser follows at most two
more pages to find visible results, including initial Home and continuation
responses. An exhausted filtered page says that members-only videos are hidden
instead of triggering the empty-Home service-recovery path. Explicit service
notices are preserved, and any remaining continuation is kept.

Access-denied playback responses now report membership, private-video,
age-verification and sign-in restrictions instead of the generic no-format or
possible-IP-blocking error, even when YouTube omits video details. Messages use
only recognized playability fields and static guidance; titles and arbitrary
response URLs do not establish access restrictions or enter diagnostics.

For actual format failures, playback tries at most two additional native
clients, embedded and Safari, reusing its HTTP client, proxy, cookies, visitor
and watch/player context. Successful direct playback keeps its request count.
Download acquisition and explicit extractor client lists do not opt in.
Candidate streams retain quality, codec/language and initialization/index
metadata; HLS recovery reads the master catalog. Wrong-video responses are not
played. Recovery player API HTTP 429 stops immediately and retains Retry-After;
exhausted recovery preserves the original solver cause. Storyboards alone are
not playable media. No Python fallback or executable installation is added.

The reported video's follow-up screenshot identified **Is Yvonne Happy with
Her New Laptop?**, from Linus Tech Tips. Its [public catalog entry](https://watch.plex.tv/en-GB/show/ltt-floatplane-exclusives/season/2026/episode/78)
lists it under LTT Floatplane Exclusives. This supports a subscription-content
hypothesis but does not establish the current YouTube upload's entitlement.
No exact video URL or YouTube playability response was provided.

## Evidence

- Eight focused offline tests pass. They cover classic video/Music renderers,
  modern lockup/thumbnail badges, public-content false positives, duplicate and
  locked-card handling, category/shelf/notice preservation, initial Home without
  redundant bootstrap requests, filtered continuation/cache behavior and the
  bounded follow-up budget. The HTTP cases use only synthetic loopback data.
- Workspace formatting and `git diff --check` pass.
- `cargo test --workspace --locked --offline`: **442 passed, 0 failed,
  31 explicitly ignored**.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`:
  passed.
- Vendored `mobile_playback`: 25 passed, including nine new recovery/access
  regressions covering paired 1080p, audio/index/language retention, HLS,
  Retry-After, wrong-video rejection, access restrictions and safe diagnostics.
- Vendored YouTube library tests: 21 passed with installed Node.js 24.19.0.
  The real Node saved-player vectors and permission checks ran; Deno-only test
  bodies were not enabled.
- Direct vendored-library Clippy with `-D warnings` reports the pre-existing
  eight-argument `extract_acquired` function (`clippy::too_many_arguments`).
  No lint allowance was added. Strict workspace Clippy passes.
- Final verification uses current main `5dcdb19` and the renamed Vidcue crates.
  The initial recheck exhausted disk space while old and renamed build artifacts
  coexisted. Removing generated incremental caches and setting
  `CARGO_INCREMENTAL=0` resolved that environment failure; the full rerun passed.

## Not verified and limits

Filtering depends on membership metadata supplied by YouTube. An unmarked
restricted upload cannot be identified from its title alone; the earlier
access-error fix still handles a direct playback denial. No live YouTube,
Android APK build, emulator or physical-device check was run for this change.
This is a source change and does not update an already installed APK.
Live YouTube/CDN hosts are outside the environment's configured destinations.
Real membership/account, physical playback and M9 release acceptance remain
unverified. No APK was installed, signed or published.

## For the plan and parity tables

M9/M2: shared browsing hides recognized members-only video/song cards, including
classic and modern cards, while preserving pagination, categories and public
content. M9/M5: native playback distinguishes access denials from format/solver
failures and uses bounded alternative-client recovery for supported streams.
Fixture/unit evidence covers filtering, pagination, recovery and safe access
errors. Live account/device and APK release qualification remain open.
