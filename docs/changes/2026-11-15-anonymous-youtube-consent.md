# Signed-out YouTube requests answer the consent page (M9)

- **Date:** 2026-11-15
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** shared engine (Android uses it)
- **Status:** fixture/unit verified; emulator verified (Android, signed out, from Germany)

## What it does
Signed out, YouTube redirects visitors in the EU and UK to its cookie-consent page
(`consent.youtube.com`). The engine treated that redirect as "sign-in required", so on Android the
Music tab, music search, playlists and playback all showed "Sign in to see this" for anyone
signed out in those regions. Requests without a session now carry `SOCS=CAI`, the cookie YouTube
sets when a visitor chooses "Reject all" on that page. It identifies no one and carries no
account. Requests with a session are unchanged: they send the session's own cookies.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Redirect reproduced | Anonymous `GET https://music.youtube.com/?hl=en` from a German host | 302 to `consent.youtube.com` | live (anonymous) |
| Cookie answers it | Same request with `Cookie: SOCS=CAI` | 200 with the InnerTube client config | live (anonymous) |
| Provider tests | `cargo test -p vidcue-providers --lib` | see the PR | fixture/unit |
| Android, signed out | Debug APK with this engine on an API 35 emulator: Music tab, a playlist, playback and music search | content loads; before the change each showed "Sign in to see this" | emulator |

The anonymous fixture tests now require that the only cookie on a signed-out request is
`SOCS=CAI` (previously: no cookie at all), so a session cookie leaking into a signed-out request
still fails them.

## Not verified and limits
- Not checked from other EU countries or the UK, or on a physical device.
- If YouTube changes how consent is recorded, the redirect returns and the existing
  "consent required" message is shown again.

## For the plan and parity tables
- Shared engine: signed-out YouTube and YouTube Music requests send YouTube's "reject all"
  consent answer (`SOCS=CAI`), so EU/UK users see public feeds, search and playback without
  signing in (2026-11-15).
