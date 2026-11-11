# M9: movie/series discovery and complete episode metadata

## Cause and change

Android Movies searched only its built-in test/curated titles. Its two curated TV
entries held S1E1 placeholders. Desktop keyless search depended on Wikipedia and
Wikidata, and loading season/episode lists required a user-supplied TMDB key.

The shared `vidcue-providers::movie_catalog` adapter searches Cinemeta's movie and
series catalogs and loads full title metadata on demand, without an embedded API
key. Metadata discovery is independent of Cinecat stream resolution. Requests
have timeouts and bounded response bodies; search extra parameters are encoded.
One failed catalog preserves the other catalog's results and exposes a warning.
Both failed catalogs produce a retryable error rather than a false empty result.

Android debounces queries, cancels superseded work, merges local and remote
results, and loads complete episode lists before opening a remote title. It
retains specials and orders ordinary seasons first, preserves legacy title/resume
keys, and persists full metadata alongside the existing Movies preferences so
resume/download entries survive process death. A failed refresh can reuse saved
metadata. Missing playback IDs or episode listings show an error/retry rather
than manufactured episodes.

Desktop keyless search resolves metadata with at most four requests in flight,
preserves the configured proxy, and supplies season/episode selection without a
TMDB key. Failed/partial metadata produces visible feedback. Explicit TMDB-key
search remains supported. Presentation work is outside this change.

## Integration

`fix/m9-catalog-discovery` branches from `origin/main` at `207767f`, which already
contains the Rune desktop app and subsequent updates. It supersedes a fix only
on the historical `t3/rune-desktop-app` branch. Android integration preserves
main's newer poster/backdrop and SearchField/continue-watching behavior.

## Evidence and limits

- Offline shared-provider fixture tests: movie/series search and detail lookup,
  full episode payloads, media-ID validation, reserved query-character encoding,
  partial outage warning and complete outage failure.
- Explicit opt-in public metadata check: `cargo test -p vidcue-providers
  live_cyberpunk_discovery_and_episode_listing --lib -- --ignored` passes. It
  discovers Cyberpunk: Edgerunners (`tt12590266`, TMDB 105248) and verifies S1E1–10.
- Android engine movie tests: 9 pass; 2 existing live stream tests remain ignored.
- Desktop movie suite on current-main integration: 16 pass, including catalog
  parsing across ordinary seasons and specials.
- Shared-provider suite on current-main integration: 2 pass; the public test is
  ignored by default and was separately run explicitly as described above.
- Android catalog suite on the original working branch: 10 pass, using local
  JDK 17 and the Android SDK. An initial attempt with system JDK 27 failed in
  Android's JdkImageTransform; using the existing local JDK resolved that toolchain
  issue.
- Android current-main integration: `:app:testDebugUnitTest --tests
  '*MovieCatalogTest' --tests '*MoviePosterTest'` passes all 12 catalog tests,
  including main's existing artwork tests. Native APK packaging, instrumentation
  and physical-device playback were not run for this change.

Public metadata can include announced episodes and incomplete title records.
A listed title/episode does not establish that a stream exists, is playable, or
is available in the user's region. No all-titles or all-episodes playback claim
is made. Device/emulator UI acceptance and release installation are separate.

Metadata protocol reference: [Stremio meta object](https://stremio.github.io/stremio-addon-sdk/api/responses/meta.html).

## PR delivery verification

Repository-required delivery checks pass: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`cargo test --workspace --locked`. Android app and instrumentation source compile
also pass with the existing local JDK 17 and SDK:
`:app:compileDebugKotlin :app:compileDebugAndroidTestKotlin`.
The strict lint check exposed the superseded Wikipedia search helper as unused
in production. It and its search-only dependency are now test-only, preserving
the existing opt-in live test without suppressing warnings or changing tests.

PR #157 independently changes Movies browsing/detail presentation in
`MoviesPage.kt`. Preserve that presentation alongside this change's top-level
search, asynchronous detail loading, retry and saved-metadata wiring when
integrating either merge order. Neither PR depends on the other.

## For the plan and parity tables

M9 movie/series discovery now uses shared public metadata rather than Android's
built-in pool or desktop's keyless Wikipedia search. Full season/episode metadata
is fixture/unit verified on both platforms; public Cyberpunk metadata discovery
is live verified. Playback, device UI, native APK packaging and installed-release
acceptance remain separate open gates.

### Integration after PR #157 merged

PR #157 merged as `a20b8ad` while this PR was being filed. Merged main once to
resolve the resulting MoviesPage conflict. Retained its filter/grid/detail
presentation and this PR's search/loading/error state. Moved the title-loading
status before the search-versus-browsing branch so it remains visible in either
mode. No catalog or playback behavior was replaced by the conflict resolution.

Post-merge Android app/instrumentation-source compile and all 12 catalog tests pass.
