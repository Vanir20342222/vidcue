# M9 follow-up: browser-derived Cinecat provider repair

The owner asked to inspect the real browser and get the providers working. The
initial adapter incorrectly treated `/sources` as the whole provider list. A
fresh incognito browser session exposed six providers: ZCDN, Aphrodite, and the
four Savannah API providers. The standalone backend now implements the two
ordinary web-provider HTTP contracts as well as the existing dynamic API list.
Android UI/JNI/playback, installed state, system configuration and VPN settings
are unchanged. No upstream provider implementation is copied into the repository.

## Root causes and changes

- ZCDN was missing. Cinecat's browser played the requested movie through
  `/cdn?tmdb=438631&type=movie`, returning `{ready:true,url:...}`. TV adds numeric
  season/episode and uses `type=tv`. A 202/queued response is not a playable stream.
  Native resolution now implements this contract. Configurable public service
  origins match the deployed site; the CLI enables web providers by default and
  offers `--api-only` for API fixtures/compatibility.
- Aphrodite was missing. It uses `/content/movie/{id}` and
  `/content/tv/{id}/{season}/{episode}`, returning `{found:true,type,url}`.
  Without the site's normal public Origin/Referer headers, its media URL redirected
  to a YouTube HTML page. Supplying those headers returned HLS. The descriptors now
  preserve that public identity; no account/session credentials are involved.
- **Valid HLS was insufficient evidence.** Aphrodite's 1080p H.264/AAC stream
  decoded successfully but displayed an Atlantic network-block card (error code 4),
  referring to Cloudflare WARP. Different movie/TV requests returned that same asset.
  The initial successful manifest/decode/network estimates were therefore not movie
  success. Known SHA-256 fingerprints of the observed error-video segments now fail
  probing/downloading before publication. This recognizes the observed asset only,
  not all possible future error videos or wrong-title content.
- Added Granite subtitle discovery using its public movie/episode catalogue.
  Explicit download language selections attach a matching external version when
  the stream advertised no sidecars. The CLI defaults to Granite, retains optional
  Wyzie/key support, and can limit downloaded versions. Language match alone does
  not establish release synchronization.
- Added explicit `--sample-seconds` qualification excerpts, rounded to whole HLS
  segments and marked `is_excerpt:true`. Ordinary downloads remain complete.

The web-contract regressions failed with HTTP 404 before routing was added; the
identity regression failed with missing headers before the descriptor fix. A
transport-level synthetic error-asset regression then failed because the error
video was incorrectly qualified, and passed after rejection was added. No test
was weakened to make an external provider appear healthy.

## Live results on this machine/network

The original Sintel browser test also failed (ZCDN queued, Aphrodite absent, four
API errors). Using an available movie and episode exposed the missing working
route. Browser video for the movie reached readyState 4 and advanced past 11 s
through ZCDN. Browser source calls also resolved movie/TV descriptors for both web
providers. The four API providers failed in that same browser context.

Native movie check (TMDB 438631), after the error-asset fix:

- **ZCDN:** 1080p, six real segments, 25,036,864 bytes representing 36.203 s.
  They arrived in 15.926 s while a separate download was running: 2.27× delivery
  headroom, zero estimated buffer stalls. An earlier isolated sample achieved
  approximately 4.4×; these are short delivery observations, not full-film guarantees.
- **Aphrodite:** descriptor resolved; actual transfer rejected as the known
  network-block error video. It is not counted as a working movie provider.
- **Abyssinia, Calico, Zanzibar, Garfield:** HTTP 502, no media descriptors. An
  explicit native API request with the site's public headers also returned 502,
  so the Aphrodite header repair did not repair the API failure.

The all-provider check correctly exits 1. The sanitized final report is
[`2026-11-09-cinecat-browser-movie.json`](2026-11-09-cinecat-browser-movie.json).
The final release-binary TV check (TMDB 1396, S1E1) also delivered native ZCDN media:
a three-segment sample represented 18.018 s and arrived in 5.193 s. See
[`2026-11-09-cinecat-browser-tv.json`](2026-11-09-cinecat-browser-tv.json).
Aphrodite's TV sample was rejected as the same
network-block asset, not evidence of that episode. Four API requests again failed.
No signed stream URLs, authentication headers, private browser state or movie
assets are included in Git.

## Actual downloads, captions and decoding

A native ZCDN qualification excerpt plus an English Granite caption completed:
32,261,640 bytes, 60.227 s, explicitly marked as an excerpt. The receipt and files
remain temporary local test artifacts; no full-length public movie was downloaded.
The public Granite catalogue returned 99 caption entries for this movie. The
selected English UTF-8 WebVTT file downloaded and ffprobe recognized a subtitle
stream. Caption content/timings were not changed, and release alignment remains
unqualified.

FFmpeg decoded the saved ZCDN video/audio with only `file,crypto,data` protocols
allowed, with exit 0. It emitted one non-monotonic-DTS warning at video frame 149;
this warning is retained here and is not described as flawless decoding. A decoded
frame showed actual studio imagery rather than the network-block error card.
Full title identity, long-session A/V synchronization, seeking and physical player
acceptance remain separate from the short network/decoder checks.

An Aphrodite excerpt captured before the guard was added decoded as 1920×1080
H.264/AAC but visually showed the error card. After the guard, the same live
download exits 1 with the explicit block reason and leaves no published output.
No network/VPN setting was changed to work around that restriction.

## Validation

- Default focused Cinecat tests: 24 passed; three explicit opt-ins remain ignored.
- Offline generated MPEG-TS/fMP4/AES-128 decoder regression: passed explicitly.
- Public web-manifest-only probe: passed explicitly; this does not qualify the
  movie behind a manifest, as the error-video investigation demonstrated.
- Public sources-API site-identity probe: failed with HTTP 502, retained as an
  external gate rather than turned into an offline expected-success test.
- `cargo test --workspace --locked --quiet`: 324 passed, 0 failed, 18 explicit ignores.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: clean.
- `cargo build --workspace --release --locked`: passed; the standalone binary is
  `target/release/vidcue-cinecat`. No APK, release, Git commit or PR was created.

The full request for every provider working remains unmet: one route now delivers
media, one is blocked by its upstream network policy, and four fail server requests
in both browser and native contexts. The backend supports them independently and
keeps their real failures visible. Broader content/device/M9 gates are still open.
