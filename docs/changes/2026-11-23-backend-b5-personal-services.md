# Self-hosted backend: health log, stats, recommendations and web admin (B5)

- **Date:** 2026-11-23
- **PR:** none yet
- **Milestone:** B5 (docs/backend/PLAN.md), contract docs/backend/PROTOCOL.md §1 and §6
- **Platforms:** backend (`apps/server`)
- **Status:** fixture/unit verified (not deployed, no app client yet)

## What it does
- **Playback health log** (`health.rs`). Devices upload sanitized outcome reports. Each report has a fixed kind, category and platform, a version string and a time; any other field rejects it, so URLs, titles, ids and error text cannot be stored. Users see their own summary and admins see all users, broken down by app version. Reports are kept for 90 days.
- **Stats and yearly review** (`stats.rs`). Both are computed from the user's own synced `history` events: per-period totals, per-kind numbers, top creators and items, and per-day and per-hour figures in the user's time zone. The yearly review adds months, the busiest day, the longest streak, the first play, new creators and the music share. Reviews are cached per user and year.
- **Private recommendations** (`recs.rs`). Off for each user until they turn them on, and switchable per server. Items come from three sources: new uploads of followed channels, items related to recently finished plays, and songs by artists similar to the user's top artists (MusicBrainz + ListenBrainz). The server fetches public pages only, with no session and nobody's cookies. Results are cached for all users, requests to each upstream are spaced out, and there is a global hourly request budget. The upstreams sit behind traits so the tests run offline.
- **Admin** (`admin_web.rs`). New JSON endpoints let the app list and revoke a user's devices and disable or enable a user. A disabled user cannot sign in or use any token, and the hourly cleanup keeps their account. There is also a `/admin` web page with no JavaScript: username/password/code sign-in, an `HttpOnly; Secure; SameSite=Strict` session cookie, CSRF tokens, Origin and Sec-Fetch-Site checks, and a CSP that pins the stylesheet hash. It shows users, one-time invite and reset links with QR codes, and the 7-day health table.
- **Migration 0005** adds `users.disabled_at`, the health, stats, recommendations and admin-session tables, and the history `at` index.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Server tests | `VIDCUE_TEST_DATABASE_URL=postgres://…/vidcue_test_b5 cargo test -p vidcue-server -j 2` | 48 passed (all B1/B2 tests unchanged, plus 5 new DB tests and 22 new unit tests) | fixture/unit |
| Offline run | `cargo test -p vidcue-server -j 2` (no database) | 48 passed, DB tests skip | fixture/unit |
| Lint | `cargo fmt --all -- --check`; `cargo clippy -p vidcue-server --all-targets -j 2 -- -D warnings` | clean | — |

New tests:
- **Unit tests:**
  - every health enum value is accepted, and unknown values, unknown fields, `at` and `duration_ms` edges and version patterns are rejected;
  - calendar arithmetic, clamping, the `seconds < 5` rule and time-zone bucketing;
  - yearly review streaks, months and an empty year;
  - the MusicBrainz and ListenBrainz parsers on hand-made fixtures (`apps/server/tests/fixtures/recs/`), including the dataset-wrapped shape and garbage input;
  - Lucene phrase escaping, the playable-item check, the server copy of the filters, and round-robin with dedupe and cap;
  - HTML escaping, the CSP hash matching the inline `<style>`, the CSRF compare, cookie attributes, and QR SVG markup without `style=`.
- **DB tests** (`src/tests/personal.rs`):
  - **Health:** per-report rejection, the 100-report limit, summary and admin aggregate, the 24 h cap (2000), the per-device upload throttle (30/h) and 90-day retention.
  - **Stats:** period stats and the yearly review against pushed history: UTC+1 bucketing, ignored short plays, clamping, `new_creators` across years, cache reuse and one row per year, empty year, and 400 errors in the `{"error"}` shape.
  - **Recommendations,** with fake upstreams:
    - Off by default. Settings validation. The 409 before enabling.
    - Saved, played, keyword- and channel-filtered, live, playlist and duplicate items are dropped, and the sources interleave round-robin.
    - A failing channel is skipped. The second build is served from the shared cache, and the expected cache keys exist.
    - Budget exhaustion stores a partial feed.
    - Background refresh returns 202, and a second refresh within 30 min returns 429.
    - Turning recommendations off deletes the feed and the rotation row.
    - Maintenance expiry works.
    - With `VIDCUE_RECS=off`, every endpoint answers 404 and `recs_available` is false.
  - **Web admin:**
    - A non-admin gets the same 401 page as a wrong password.
    - Foreign `Origin` and `cross-site`/`same-site` fetch metadata are refused.
    - Cookie attributes are correct and only the token hash is stored.
    - The dashboard leaks no tokens, secrets or recovery codes.
    - Missing or wrong CSRF and missing sessions change nothing.
    - Invite and reset links are shown once, with a QR code.
    - At most 3 sessions per admin. Idle expiry clears the cookie, maintenance removes the session, and logout works.
    - The login throttle uses `admin-` keys and leaves `/api/login` unaffected.
  - **Admin JSON:**
    - Device listing.
    - Tree revocation and revoke-all.
    - A self-disable gets 409.
    - Disabling refuses tokens, login (app and web), open pairings, `account::user_id` and admin reset links (409 "Enable the user first"). `admin_users` shows `disabled`.
    - Cleanup keeps disabled accounts.
    - Enabling restores sign-in.
    - A disabled admin cannot act.

## Not verified and limits
- **Nothing live.** No real YouTube, MusicBrainz or ListenBrainz request was made.
  - The ListenBrainz `algorithm` value matches one listed on the public labs.api.listenbrainz.org form. The response shape is parsed defensively (flat array, or `[{data: [...]}]`) and has not been checked against a live answer.
  - Channel uploads use `BrowseRequest::Collection(UC…)` on the web client, which the provider's channel test confirms opens the Videos tab.
- **Contract deviations (smallest sensible):**
  - **Change note name:** `2026-11-23-backend-b5-personal-services.md`, as the task asked, instead of `…-b5-personal.md`.
  - **Module order:** `rustfmt` sorts the `mod` lines in `main.rs` alphabetically, so the B5 modules are not literally "directly after `mod auth;`".
  - **`top_categories`:** lists non-`ok` categories only (the problems).
  - **`ok_rate`:** `null` when a kind has no reports.
  - **Empty yearly review:** `busiest_day` and `first_play` are `null`.
  - **New throttle `STATS_COMPUTE`:** 60 computations per user per 10 min. It is not in the contract but bounds CPU and database work; cached yearly reviews don't count against it.
  - **`stats_wrapped`:** keeps one row per user and year. Asking with another offset replaces it, so the table cannot grow once per offset.
  - **Recommended items:** only videos and songs, so playlists and channels from related pages are dropped.
  - **`PUT /api/recs/settings`:** accepts partial bodies. `POST /api/recs/refresh` answers 409 while recommendations are off.
  - **Shared limit:** the "last build < 30 min" rule is a persistent throttle row (`recs-build:<user>`) shared by automatic and manual builds.
  - **Per-user lock:** the per-user lock is a transaction-level advisory lock held during the build. The server-wide limit of one build is an in-process semaphore.
  - **Upstream budget:** counts trait calls. One YouTube call may make an extra bootstrap request the first time the provider client is used.
  - **Web admin page:** POST actions render the dashboard directly, with the result above it, instead of redirecting. That way one-time links appear exactly once.
  - **Login checks:** Origin and Sec-Fetch-Site checks also apply to `POST /admin/login`.
- **Last-admin protection:** a self-disable is refused, and the check runs under row locks on every enabled admin. Since the actor must be an enabled admin other than the target, the "last enabled admin" branch is only reachable through races. Tests cover the self case and the "disabled admin cannot act" case.
- **`/admin` behind a proxy:** works only when `VIDCUE_PUBLIC_URL` is the exact public origin, because the Origin check compares against it. The `Secure` cookie needs HTTPS (or localhost).
- **Deferred per contract:** web admin pagination, and hiding disabled users from friends and Jam (needs B4).

## For the plan and parity tables
- B5 server (health log, stats + yearly review, opt-in private recommendations, admin JSON + `/admin` web page, user disable): implemented, fixture/unit verified on 2026-11-23. Not deployed, no client yet, no live upstream check.
