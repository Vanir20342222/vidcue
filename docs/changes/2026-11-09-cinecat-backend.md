# M9 follow-up: independent Cinecat backend

The owner requested a backend independent of Android, all providers working,
smooth delivery on this machine's network, downloads and subtitles. The library
and CLI implement the sources-API transport and offline transfer path. **The full
requested live outcome is not achieved:** all four advertised provider resolution
requests returned HTTP 502, and browser-only scraper adapters are not implemented.

## Implementation

- Added `vidcue_providers::cinecat` and workspace tool `vidcue-cinecat`, without
  changing Android screens/JNI/playback/settings, system configuration or releases.
- Dynamic provider enumeration, movie/TV resolution, playback descriptors and
  independent per-provider network qualification with sanitized JSON reports.
- HLS quality selection, separate default audio, subtitle renditions, byte ranges,
  init segments, discontinuities and standard AES-128; progressive-file downloads.
- Private staging, bounded memory/concurrency/bytes, limited transient segment
  retries, cancellation/cleanup, and atomic non-overwriting Linux publication.
- Provider caption sidecars, HLS WebVTT, external Wyzie-compatible search using a
  caller-supplied key, and atomic content-validated UTF-8 SRT/WebVTT downloads.
- No public HTTP server, Android dependency on the tool, browser-cookie access,
  executable auto-installation, access-control bypass or copied upstream code.

The complete library/CLI contract, commands and unsupported cases are in
[`tools/cinecat/README.md`](../../tools/cinecat/README.md).

## Live evidence: 2026-11-09, this host/network

Public site configuration pointed `VITE_SOURCES_API_URL` to
`https://edge.cinecat.eu`. The shipped source adapter used `GET /sources` and
`GET /sources/{name}?tmdb={id}&type=movie` (TV additionally uses season/episode).
The website also shipped a separate browser scraper library; the API catalog
must not be presented as the entire website provider list.

The related [public P-Stream repository](https://github.com/xp-technologies-dev/p-stream)
explicitly excludes its providers from the open-source code. Its
[package manifest](https://raw.githubusercontent.com/xp-technologies-dev/p-stream/production/package.json)
points to the separate `xp-technologies-dev/providers` repository. This is not a
verified source/license for the deployed, obfuscated Cinecat scraper bundle, so
that bundle was neither vendored nor executed as a substitute for native adapters.
Also, the currently shipped Apps-page code says Cinecat's native apps are
deprecated; the earlier search-index description of active native downloads was
stale and is not evidence of current native compatibility.

`vidcue-cinecat sources` succeeded and advertised Abyssinia, Calico, Zanzibar and
Garfield in the Savannah group. `vidcue-cinecat check --tmdb 45745 --segments 6`
(Sintel) independently attempted each and exited 1: all four returned HTTP 502,
no stream descriptors, and no media samples. Resolution attempts in the recorded
CLI run took 95–191 ms. See the sanitized
[`2026-11-09-cinecat-live.json`](2026-11-09-cinecat-live.json).
An additional Abyssinia request for Big Buck Bunny (TMDB 10378), including normal
site Origin/Referer headers, also returned 502. No inference is made about whether
the origin, intermediary or this environment caused those responses. The catalog
includes a private/paid-use notice; no supported public API contract or client
authorization was established. No credentials or restrictions were bypassed.

No Cinecat media/subtitle URLs were obtained or committed. No successful live
playback, full download, subtitle match or full-length network-smoothness result
is claimed. External paid subtitle lookup remains unqualified without credentials.
This work does not close M9's mobile, physical-device or service acceptance gates.

Public resources inspected (original bodies retained only in temporary storage):

- `https://cinecat.eu/config.js`: SHA-256
  `6c74ae9fa9b58e0e04f55766ffbc095cbd99ad1e6c5d5db4104a3b88cb094e1e`.
- `https://cinecat.eu/_app/immutable/chunks/CiGQyRlw.js`: SHA-256
  `d1b5c376993ca3ebd604e768cd582562aa037efe5a4947000c3029d651776e8f`.
- Public subtitle-search contract inspected in the shipped `BT-4MXYE.js` chunk.

## Validation

Environment: rustc 1.99.1; FFmpeg n9.0.2; Linux. Focused Cinecat qualification passed all 19 tests,
including the explicit installed-FFmpeg regression. Actual generated MPEG-TS,
fMP4 and AES-128 HLS, separate audio and subtitle assets were downloaded through
loopback HTTP; the server was stopped and FFmpeg decoded the downloaded packages
with network protocols disallowed. This is local decoding evidence only.

Final checks actually run:

- `cargo fmt --all -- --check` and `git diff --check`: clean.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked --quiet`: 318 passed, 0 failed, 16 explicit
  ignores (including the opt-in Cinecat decoder test).
- `cargo test -p vidcue-providers cinecat --lib -- --include-ignored`: all 19
  focused tests passed, including the explicitly run real decoder case.
- `cargo build --workspace --release --locked`: passed; standalone binary is
  `target/release/vidcue-cinecat`; its `--help` command passed.
- Live `sources`: passed; live all-provider `check`: correctly failed with
  exit 1 and the four HTTP 502 results described above.

An initial strict Clippy run caught an unchecked read count in a fixture; it was
fixed and the final strict workspace run passed. No test was weakened. No APK,
installed desktop integration, release, Git commit or PR was created.

Regressions cover per-provider failure visibility/redaction, future provider IDs,
TV/specials, malformed/oversized responses, relative redirects/header scoping,
key/range/init handling, ignored byte ranges, interrupted transfers, cancellation,
transient retries, byte budgets, existing-output protection, subtitle Unicode/
content validation and rollback, and unsustainable-delivery rejection despite 200s.

Remaining work: restore/obtain authorized live API access and rerun representative
movie/TV corpora for every provider; add independent browser-only scraper/embed
adapters if permitted; qualify actual captions and long network/decoder sessions.
The current 1080p short-sample network estimator is not automatic player adaptation.
