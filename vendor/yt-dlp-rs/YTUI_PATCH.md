# ytui Android runtime adapter

Upstream: https://github.com/aagedal/yt-dlp-rs at
`f67d96bdf98c5b821d0c9bf6175d328a0fa8ceab`, under its Unlicense.
Production source, EJS licenses and upstream tests are retained.

The runtime adapter adds `set_embedded_js_evaluator` and the explicit
`ytui-android-js` runtime sentinel. An Android caller runs the original bounded
EJS batch in AndroidX JavaScriptSandbox, without network, cookie or host APIs.
The original batch validation still enforces response identity and size limits.
Desktop Node/Deno execution and explicit-runtime validation stay unchanged.

The callback must impose a deadline and isolate teardown. Android's bridge uses
15-second deadlines and a 128 MiB heap limit. This hook is not runtime discovery
and does not install any executable. Rebase/remove the patch when upstream
accepts an equivalent embeddable runtime API.

Fixture-only sanitization replaces Google's two public client keys in both saved
player scripts with inert, equal-length placeholders. This avoids publishing API
key matches in test assets; challenge algorithms and exact solution maps remain
unchanged. See `docs/SECURITY_REVIEW_2026-10-27.md` at the repository root for the
provenance review and precise historical scanner baseline.

M9 playback-latency follow-up also retains public, versioned HTTPS
`/s/player/` JavaScript in a bounded in-memory cache (three versions, 8 MiB,
six-hour lifetime, four MiB per response). Duplicate requests coalesce per URL;
independent versions never hold a global lock during HTTP I/O. Account/player
API replies, cookies and signed stream URLs are not stored in this cache.
Challenge algorithms and validation remain unchanged.

M9 live-playback follow-up permits on-air YouTube broadcasts only in the opt-in
playback extractor. It selects rolling HLS variants and leaves playlist refreshes
and segment loading to Media3. Live HTTPS sequence URLs are excluded, and their
advertised quality does not trigger VOD direct-format recovery. Initial playback,
audio playback, full reacquisition and alternative-client recovery are covered by
the offline `mobile_playback` suite. Upcoming/post-live processing and live
downloads remain unsupported; ordinary VOD/download extraction is unchanged.
