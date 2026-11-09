# M9 follow-up: default server with backup failover

The owner asked for a default server with backup servers "so playback is
guaranteed 99 percent of the time". This change implements automatic provider
failover in the Cinecat backend. It does **not** establish a 99 percent success
rate: only one provider (ZCDN) currently delivers real media on this network, so
live failover was demonstrated with an explicit order, not by several healthy
backups racing each other. The mechanism is in place; the number depends on how
many upstream providers eventually serve real media.

## Changes

- The site's own config (`window.__CONFIG__`) rotates playback between three
  independent ZCDN worker origins (`zcdn1`, `zcdn2`, `zcdn3`). All three were
  verified live against `/cdn?tmdb=...` and are now separate failover providers:
  `zcdn` remains the alias for the primary origin (default zcdn2, unchanged),
  `with_zcdn_backup(name, origin)` adds `zcdn1`/`zcdn3`, and the default
  failover order is `zcdn, zcdn1, zcdn3, aphrodite` followed by the API
  providers. Duplicate origins/names and catalog name collisions are rejected.
- `Backend::resolve_auto(media, order, verify, cancel)` tries providers in order
  until one serves the requested media. The first candidate is the default; the
  rest are backups. Explicit orders are validated (identifier rules, duplicates,
  empty list, 64-entry cap).
- Default order without an explicit `--order`: the configured web providers
  (ZCDN first, then Aphrodite), then every advertised sources-API provider. The
  catalog is fetched only after the web candidates are exhausted, so a healthy
  web provider never waits on an API listing that may itself be down. The
  catalog fetch failure is recorded as a `sources-catalog` attempt and is not
  hidden.
- Optional verification (default on in the CLI): a candidate only wins if every
  returned stream actually delivers three real segments. This is the lesson from
  the error-video investigation — a resolved descriptor or valid manifest is not
  playback. The known network-block error video and dead manifests now fail
  over to the next provider instead of being handed to the player. `--no-verify`
  skips the sampling cost.
- Every attempt is reported in a sanitized `failover` array (provider, success,
  elapsed, error). Errors are the same sanitized strings used elsewhere; no
  URLs, headers or response bodies are emitted.
- CLI: `resolve` and `download` accept an omitted `--provider` for automatic
  failover, plus `--order a,b,c` and `--no-verify`. Explicit `--provider` keeps
  the previous single-provider behavior unchanged.
- Failover never retries the same provider and never routes around 429 rate
  limits; it moves to the next provider. Rate limiting of one service is not an
  excuse to hammer it through another route.

## Offline validation

- `failover_skips_failed_providers_and_reports_every_attempt`: 502 default is
  skipped, the backup wins, attempts are reported, and an order where every
  candidate fails exits with an explicit error.
- `failover_verification_rejects_error_video_and_falls_back_to_backup`: a
  synthetic blocked error asset on the default provider is caught during
  verification and the backup wins; without verification the blocked default
  would have won.
- `failover_order_is_explicit_validated_and_wins_over_defaults`: explicit order
  is honored; invalid identifiers, duplicates and empty orders are rejected.
- `cargo test --workspace --locked --quiet`: passed, 0 failures.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo fmt
  --all -- --check` and `git diff --check`: clean.
- `cargo build --workspace --release --locked`: passed.

## Why the other five providers remain blocked (investigated further)

- The four sources-API providers (Abyssinia, Calico, Zanzibar, Garfield) still
  return HTTP 502 on `/sources/{provider}` per-provider requests, with or
  without the site's public headers, and via POST (405 confirms the route).
  The catalog itself is up. Cinecat's own frontend bundles a persistent
  "provider issues" downtime notice for exactly this situation, so the failure
  is server-side and not fixable from a client.
- Aphrodite's video CDN serves a Cloudflare WARP network-block error card
  (error code 4) to this network; the real browser saw the same card, so this
  is network-level, not a header or client bug. The site's `zstream-proxy1..3`
  workers reject an unencrypted `?payload=` request with
  "Encrypted payload required" (400 for the plain shapes, 403 for base64 JSON),
  and the payload scheme is not present in the publicly served JavaScript.
  Following the full SvelteKit chunk graph found no decryptable proxy
  construction. No VPN or system network setting was changed to work around
  the block.

## Live results on this machine/network (2026-11-09)

- Default auto resolve, movie TMDB 438631: ZCDN won after verification
  (3.191 s of segment sampling); no backup was needed.
- Independent ZCDN origins: `zcdn1` and `zcdn3` both serve the same movie
  (`/cdn` returned `ready:true` on all three workers). A failover run with
  order `aphrodite,zcdn1` correctly skipped the blocked Aphrodite and ZCDN1
  won after verification. A 30-second excerpt (19,400,837 bytes) plus an
  English Granite caption downloaded through the `zcdn3` backup alone,
  proving each origin delivers independently.
- Explicit failover order `aphrodite,zcdn`, same movie: Aphrodite resolved but
  failed verification with the known network-block error video, and the backup
  ZCDN won. This is the live demonstration that a broken default hands off to a
  backup rather than publishing a broken stream.
- Automatic download with failover verification, movie TMDB 438631: a 30-second
  excerpt (19,400,837 bytes) plus an English Granite caption downloaded through
  the auto-selected ZCDN stream. Receipt marked `is_excerpt:true`; the excerpt
  remains a temporary local artifact. Long-session decoding was not re-run in
  this change.
- TV TMDB 1396 S1E1 auto resolve: ZCDN verified and won (5.172 s of sampling).

## Coverage test on 2026-11-09 (44 titles through the failover backend)

Auto resolution with verification across a spread of titles: 16 mainstream
classics/blockbusters (Fight Club, Inception, The Matrix, Interstellar,
Parasite, Endgame, both Dunes, Oppenheimer, Barbie, Spider-Man NWH, The
Godfather, Spirited Away, Fury Road, EEAAO, Top Gun Maverick), 8 catalog
classics/older titles (Metropolis 1927, Nosferatu 1922, Psycho-era
neighbors, Terminator, Jurassic Park, Toy Story, Home Alone) and the public
TMDB top-20 popular list of the day (including Project Hail Mary, The Dark
Knight, Spider-Man: Brand New Day, The Odyssey, Coco, Godzilla Minus Zero).

Result: 35 of 44 resolve and deliver verified media through ZCDN. Two
end-to-end excerpts (Project Hail Mary 687163 and Metropolis 1927/19) were
downloaded and decoded with FFmpeg restricted to local protocols — both
exit 0 with real H.264/AAC at 1080p (1922×1080 and 1440×1080). TV spot
checks (Breaking Bad S1E1, Friends S1E1) also pass through ZCDN.

The 9 failures are all one upstream mode, not client bugs:

- **ZCDN "queued"**: the worker returns `ready:false` for titles whose
  transcode build is not available. Observed as `building` on first request
  then `queued` on every poll for 3+ minutes across all three worker origins
  (zcdn1/2/3) — the site's build queue is stalled for those titles. The
  site's own UI text is "queued; retry later". Client-side polling was
  deliberately **not** implemented because evidence shows no ready
  transition within minutes; adding speculative waits would only hide the
  upstream stall.
- The only alternate route for queued titles is Aphrodite, which does resolve
  descriptors for several of them but serves the known network-block error
  video on this network, so verification correctly rejects it.
- A rate-limit artifact was observed while testing: hammering the workers
  with rapid sequential requests across ~40 titles triggered HTTP 429 on
  ZCDN, which the backend reports explicitly and never retries or routes
  around. Paced re-testing recovered the rate-limited titles.

Known imperfect: the total-failure error now lists every candidate's
sanitized reason (offline regression covers it), making the queued-vs-blocked
distinction visible from a single command.

## Remaining limits

- The 99 percent framing is not met and not claimed. The coverage ceiling is
  set by upstream availability: titles without a ready ZCDN build cannot be
  forced to play from the client, and Aphrodite (their only alternate) is
  network-blocked here. Failover guarantees automatic avoidance of broken
  providers, not universal title coverage.
- Verification samples three segments per stream (~18 s of media) before
  declaring a candidate good; it does not qualify full-length playback,
  seeking, A/V synchronization or title identity.
- No APK, release, Git commit or PR was created.
