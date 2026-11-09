# Standalone Cinecat backend (M9 follow-up)

Rust library: `vidcue_providers::cinecat`. CLI: `vidcue-cinecat`. No Android UI,
JNI, player, account, installed application or user settings are changed. The CLI
is a local development tool, not an installed service or an HTTP server.

**Status: ZCDN now resolves and delivers real movie/TV media on this network.**
The browser investigation found two web providers missing from the initial API-
only adapter: ZCDN and Aphrodite. Both now have native HTTP adapters. Aphrodite's
CDN requires the site's public Origin/Referer headers; even with them it currently
serves a decodable network-block error video. Known fingerprints of that asset are
rejected during probing/downloading. The four API providers (Abyssinia, Calico,
Zanzibar and Garfield) still return 502, including in the browser. These failures
are not substituted with another provider's success.

Granite external subtitle discovery/download now works without a bundled key.
See the [browser repair and live evidence](../../docs/changes/2026-11-09-cinecat-browser-repair.md)
and the [initial API-only investigation](../../docs/changes/2026-11-09-cinecat-backend.md).

## Run

From the repository root:

```sh
cargo run -p vidcue-cinecat -- sources
cargo run -p vidcue-cinecat -- resolve --tmdb 438631
cargo run -p vidcue-cinecat -- resolve --provider zcdn --tmdb 438631
cargo run -p vidcue-cinecat -- resolve --tmdb 438631 --order aphrodite,zcdn
cargo run -p vidcue-cinecat -- check --tmdb 438631 --segments 6 --report /tmp/cinecat-check.json
cargo run -p vidcue-cinecat -- download --tmdb 438631 --output downloads/movie --max-height 1080 --subtitle-languages en
cargo run -p vidcue-cinecat -- download --provider zcdn --tmdb 438631 --output downloads/sample --sample-seconds 60 --subtitle-languages en
cargo run -p vidcue-cinecat -- subtitles --tmdb 438631 --language en --limit 1 --output downloads/captions
cargo run -p vidcue-cinecat -- sample --count 15 --seed 0 --report /tmp/sample.json
```

The all-provider `check` currently exits nonzero because five providers remain
blocked on this network; the explicit ZCDN commands use the working route.
Use `--season 1 --episode 1` together for a TV request; season zero supports
specials. Global `--api` selects a compatible endpoint and `--concurrency 1..8`
limits concurrent provider checks/resource downloads (default two). `--zcdn` and
`--aphrodite` override the observed public service origins; `--api-only` disables
the web adapters. The library opts into them with
`Backend::new(api, concurrency)?.with_web_sources(WebSources::new(zcdn, aphrodite)?)`.
Nothing
automatically installs external tools or changes network/VPN configuration.

`sources` lists the two configured web adapters plus every current API provider;
no hard-coded API-provider list is used. `resolve` emits a sanitized summary. Explicit `--show-urls` emits the
short-lived playback descriptors for another local client; treat this output as
private. `check` independently resolves **each** advertised provider and reads
real segments for every returned stream. It exits nonzero if any provider fails,
any stream cannot be sampled, or delivery cannot be qualified. Reports omit
URLs, request headers, remote error bodies and API keys. Report files are created
exclusively, so choose a fresh filename on each run.

## Streaming contract and network evidence

Omitting `--provider` on `resolve`/`download` selects the automatic failover
backend (`Backend::resolve_auto`): the default provider is tried first and the
rest act as backups in order. The default order is the web providers
(`zcdn, zcdn1, zcdn3, aphrodite`) followed by every advertised API provider;
the catalog is only fetched once the web candidates are exhausted, and
`--order` overrides it entirely. By default a candidate only wins if its streams actually
deliver three real segments each, so dead manifests and the known network-block
error video fail over instead of being published; `--no-verify` skips that
sampling. The response's `failover` array records every attempt and its
sanitized error. Failover never retries the same provider and never routes
around 429 rate limits; it moves to the *next* provider. Verifying candidates
costs real segment downloads before playback, which is the price of not handing
the player a broken stream.

## Automated randomized live tests (`sample`)

`sample` draws a seeded random selection of titles from a built-in pool of
public TMDB entries (movies and TV episodes), then runs the full failover
pipeline with real-segment verification and a real excerpt download for every
title. Excerpts are throwaway qualification artifacts and are deleted after
each pass; reports contain no URLs or headers. The run exits nonzero when
fewer than `--min-pass` titles pass (default 14/15).

```sh
cargo run -p vidcue-cinecat -- sample --count 15 --seed 0 --report /tmp/sample.json
```

The pool is curated and verified resolvable when written: it tests the
backend, not the site's upstream build queue. Six seeds × 15 titles passed
15/15 on 2026-11-09; see
[2026-11-09-cinecat-sample.md](../../docs/changes/2026-11-09-cinecat-sample.md).


Cinecat rotates three independent ZCDN worker origins. `zcdn` is the primary
(default zcdn2); `zcdn1` and `zcdn3` are added automatically as backup
providers, so the default failover order is `zcdn, zcdn1, zcdn3, aphrodite`.
`--zcdn1`/`--zcdn3` override those origins (set one equal to `--zcdn` to drop
it).

Observed title-availability modes (2026-11-09, 44-title sweep): most titles
resolve and deliver verified 1080p H.264/AAC media through ZCDN, including a
1927 silent classic and current releases. Titles whose upstream transcode is
not ready return `ready:false` ("building"/"queued") from every ZCDN origin
and do not become ready within minutes; the backend reports this explicitly
("ZCDN content is not ready (queued)") and fails over, but there is nothing
to fail over to while Aphrodite is network-blocked and the API providers are
down. Total failures list every candidate's sanitized reason. HTTP 429 from
a provider is reported and never retried or routed around; space out bulk
runs (the rate limit is per worker origin, so failover still tries the other
origins).

`Backend::resolve` returns `Resolution { provider, streams, elapsed_ms }`. Each
stream carries its kind, URL, validated HTTP playback headers and advertised
subtitle descriptors. The consuming player handles playback/adaptation; this
tool does not act as a buffering/relay server.

The ZCDN adapter uses `/cdn?tmdb=...&type=movie|tv` (season/episode for TV);
202/`ready:false` reports queued content without polling or pretending success.
Aphrodite uses `/content/movie/{id}` or `/content/tv/{id}/{season}/{episode}`.
Their stream descriptors retain the public Cinecat Origin/Referer identity.

HLS master/media playlists, relative URIs (relative to the final redirected
playlist URL), external audio/subtitle renditions, init maps, byte ranges,
discontinuities and standard AES-128 are handled. Downloads choose the highest
advertised bandwidth within `--max-height`. They preserve the default separate
audio rendition and selected subtitle languages. Embedded audio/subtitles remain
inside their original media segments. AES-128 keys are downloaded through the
same validated transport; unsupported encryption/DRM fails explicitly.

`probe` selects at most 1080p, fetches up to the requested number of complete
segments and samples separate audio too. It reports actual bytes, wall time,
throughput, media seconds, delivery headroom and individual video-segment timing.
The buffer estimate starts at ten seconds; a sustainable result requires at least
three video samples, no estimated buffer starvation, and aggregate delivery
headroom of at least 1.3. It is a short network sample, **not** a guarantee of
smooth full-length playback. Decoder verification is always false in these live
network reports. Progressive sampling reads roughly the first MiB (or the complete
file if smaller), then drops the response; without media bitrate/duration it makes
no sustainability claim.

Known fingerprints of the observed network-block video are rejected before a
sample can be qualified or a download published. This catches that specific
upstream error asset, not arbitrary future error videos or wrong-title streams.
An HLS response, codec declaration or successful decode alone does not establish
the identity of the content. Visual/long-session acceptance remains separate.

## Downloads and subtitles

`download` creates a complete, relocatable offline **directory**, not a single
remuxed MP4. Open its `index.m3u8` in an HLS-capable local player. The package has
local video/audio/subtitle playlists and resources, an optional caption sidecar,
and a sanitized `download.json` receipt. No remote/signed URL is retained. For a
progressive stream the entrypoint is `video.media`, containing the original file.
The library also exposes the transfer methods independently of provider resolution.

`--sample-seconds 1..300` explicitly requests a qualification excerpt rounded up
to complete segments. Its receipt sets `is_excerpt:true`; ordinary downloads
remain complete. Excerpts require finite HLS and are not a live-recording feature.

Segment downloads are bounded and concurrent, reuse connections, and retry only
transient transfer/408/502/503/504 failures up to twice with bounded backoff.
401/403/404/429 are not retried or routed around. Byte-range responses are checked
for exact status, Content-Range and length. A size budget defaults to 20 GiB of
downloaded media/captions/keys; manifests/receipt are additional small metadata.
Individual HLS resources are limited to 64 MiB, playlists/API JSON to 2 MiB,
and external captions to 8 MiB. Progressive transfers stream to disk with an idle
timeout rather than a 45-second movie-length deadline.

Staging directories are private. Failure or Ctrl-C removes staging, while success
publishes by an atomic non-overwriting directory rename on Linux. Existing
destinations are never replaced. Downloads restart from scratch after failure;
resume, pause, disk quotas and background job persistence are not implemented.
Offline directory publication currently requires Linux; it fails explicitly on
other platforms rather than risking an overwrite. There is no Android integration.

Subtitle support includes provider-advertised UTF-8 SRT/WebVTT sidecars and HLS
WebVTT renditions. Original timings/content are preserved. Sidecar format is
detected from content; HTML/login pages, binary archives and unsupported encodings
are rejected. Caption collections publish atomically with correct `.srt`/`.vtt`
extensions. `--subtitle-languages` filters advertised captions for movie downloads;
when no sidecars were advertised, an explicit filter fetches Granite and attaches
the first matching version for each requested language. Missing requested matches
fail explicitly. With no filter, only advertised/HLS captions are downloaded.

`subtitles` defaults to the public Granite catalogue, using `/v1/movie/{id}` or
`/v1/tv/{id}/{season}/{episode}`. It retains release labels and maps common language
names to codes without discarding unknown languages. `--limit` bounds downloaded
versions. Matching language does not establish release synchronization.
`--service wyzie` selects the original Wyzie-compatible `/search` integration;
the caller's optional key is read from `CINECAT_SUBTITLE_KEY`, never bundled or
written to reports. `--endpoint` overrides the selected service endpoint/origin.
Other subtitle aggregators and paid account features are not implemented.

## Coverage boundary

The sources API and the website's browser-side scraper library are separate.
This adapter supports the two currently browser-visible web providers and **all
advertised sources-API identifiers**, including future identifiers with the same
response contract. Extension-only/third-party scraper additions are not included,
and the implementation does not imply every provider works on this network.
Unknown response kinds fail explicitly. DASH descriptors can be represented but
DASH downloading/probing, live-HLS recording, DRM and low-latency partial-segment
downloads are not implemented. No upstream JavaScript or scraper source is copied
into or executed by the backend, and no dependency/executable is downloaded at runtime.

The public catalog includes a private/paid-use notice. Exposing an endpoint does
not grant client access or a supported integration contract. The backend reports
access restrictions, rate limits and upstream errors; it does not bypass them.

## Checks

Ordinary tests are offline and require no browser cookies or credentials:

```sh
cargo test -p vidcue-providers cinecat --lib
cargo clippy -p vidcue-providers -p vidcue-cinecat --all-targets -- -D warnings
```

The installed-FFmpeg regression is opt-in and also offline:

```sh
cargo test -p vidcue-providers cinecat::tests::real_decoder --lib -- --ignored
```

It generates actual MPEG-TS, fMP4 and AES-128 media plus separate audio and HLS
subtitles, downloads through loopback HTTP, stops the server and decodes the
offline bundles. FFmpeg is restricted to local `file,crypto,data` protocols.
These fixtures establish the transfer/packaging path, not live Cinecat success.

The two `live_*` tests are separately ignored public-service probes. One checks
web-provider HLS manifests only (an error video can also have a valid manifest).
The sources-API site-identity probe currently fails with 502. Do not use
`--include-ignored` for ordinary offline checks.
