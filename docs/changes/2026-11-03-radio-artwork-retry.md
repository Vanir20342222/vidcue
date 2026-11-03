# Radio artwork: survive Wikimedia rate limits (2026-11-03)

Symptom (Galaxy Tab/phone, Radio tab): several station tiles (NCS Dubstep, Drum & Bass,
Trap, ...) stayed on the placeholder while others loaded.

Cause: genre photos come from upload.wikimedia.org. A grid fetches many at once with a
generic browser User-Agent; Wikimedia answers `429` with `Retry-After`, and the image cache
treated that as a permanent failure for the tile.

Change (`crates/ytui-providers/src/images.rs`):
- Send a descriptive `ytui/1.0 (...)` User-Agent to upload.wikimedia.org only.
- Retry `429`/`503` up to three times, honouring `Retry-After` (clamped 1-8 s).

Checks run: `cargo test -p ytui-providers images` (4 pass), `cargo fmt --check`.
Not verified: the live Wikimedia fix on a device. The CI sandbox IP was itself throttled
(429 with both User-Agents), so the effect of the UA change alone is unconfirmed.
No new offline test for the retry path.
