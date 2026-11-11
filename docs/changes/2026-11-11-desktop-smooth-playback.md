# M9: Desktop colour grading without per-frame RGB conversion

- **Date:** 2026-11-11
- **Milestone:** M9
- **Platforms:** desktop
- **Status:** fixture/unit verified

## What it does

The Rune desktop player's software-rendered video was severely throttled by
colour grading. The previous `eq,colorbalance` chain converted decoded YUV to
planar RGB and back for each frame. Earlier full-window instrumentation in this
session observed roughly 8–9 distinct video frames/second with that chain;
network buffering was false. The neutral-grade headless path delivered nearly
all frames, isolating the expensive grading path rather than network extraction.

`grade_filter` now keeps frames in YUV. A luma lookup table handles brightness,
contrast and gamma; chroma lookup tables handle saturation when no warmth is
needed. Warm/cool looks use `colorcorrect` for a small luma-dependent chroma
shift, approximating Android's red/blue gain treatment. This preserves the named
looks and settings, but is not a pixel-identical recreation of the former RGB
mid-tone colour balance. Off/unknown still sends no filter. Lookup expressions
scale with source bit depth rather than clipping 10-bit inputs to 8-bit values.

## Reproducible checks

The opt-in `player::mpv::tests::graded_1080p60_keeps_up` exercises the actual
libmpv software renderer and Rune surface with null audio, without a GUI or
accounts. It requires an explicitly supplied local 8-second 1080p60 clip:

```sh
ffmpeg -v error -f lavfi -i 'testsrc2=size=1920x1080:rate=60:duration=8' \
  -c:v libx264 -preset veryfast -pix_fmt yuv420p /tmp/vidcue-1080p60.mp4
VIDCUE_BENCH_CLIP=/tmp/vidcue-1080p60.mp4 \
  cargo test --release --locked --offline -p vidcue-desktop \
  graded_1080p60_keeps_up -- --ignored --nocapture
```

Use `VIDCUE_BENCH_GRADE=warm|cool|vivid|cinematic|off` to select a look.
`VIDCUE_BENCH_VF` overrides only the test filter to replay the old chain. With
`@vidcue-grade:lavfi=[eq=brightness=0.000:contrast=1.080:saturation=0.900:gamma=0.970,colorbalance=rm=0.040:bm=-0.040]`,
the test failed: only 95/480 frames in 15.02 seconds, without reaching EOF.
The threshold remains >=432/480 frames and EOF within 10 seconds; the benchmark
is ignored by default because throughput depends on the host's load and hardware.

Offline pixel tests exercise every named look on both 8-bit and 10-bit grey
frames through FFmpeg, checking preserved luma range and warm/cool direction.

## Evidence boundary

This change targets the desktop grade filter only. It does not change shared
provider code, the Android player, rendering architecture, installed settings,
or release versions. Full-window playback with the final build, live-service
acceptance and release qualification remain separate from headless throughput.

## Final local results

- `cargo test --release --locked --offline -p vidcue-desktop`: 89 passed,
  4 ignored (three live-service tests and the opt-in throughput test).
- Explicit throughput runs through the final production filter code passed for
  every preset: warm 470/480 frames in 8.73s; cool 444 in 9.06s; vivid 446 in
  8.70s; cinematic 438 in 8.94s. Neutral/off delivered 470 in 8.84s.
  Hardware decode reported `nvdec-copy` on this i5-8300H/GTX 1050 Ti host.
- Concurrent Android/Rust builds substantially distorted earlier measurements:
  new-filter runs ranged from 160 to 403 frames and missed the unchanged
  threshold. These failed samples are not discarded as a universal success claim;
  the later passes establish this local scenario only. There was no fully idle
  host qualification, nor a final full-window acceptance run.
- The regression pixel test first caught incorrect use of FFmpeg's nominal
  `maxval`; the implementation now derives full-scale luma/chroma code values
  separately. The grey fixture also now explicitly sets neutral chroma at each
  depth instead of relying on FFmpeg's slightly offset 10-bit `color=gray`.
- `git diff --check` passes. No temporary production instrumentation remains.
- No installed executable or OS configuration was replaced. This worktree is
  originally based on `207767f`. During delivery, `main` advanced to `a20b8ad`
  (Android Movies PR #157); the fix was rebased cleanly onto that commit. The
  Android changes are preserved. Other unmerged desktop/UI work remains
  independent; installation is outside this PR. Delivery validation uses an
  isolated Cargo target to avoid sharing executable outputs with other worktrees.

## For the plan and parity tables

M9 desktop: colour presets use YUV lookup tables and chroma correction rather
than a per-frame RGB colour-balance conversion. Offline 8-/10-bit pixel tests
and local headless 1080p60 throughput checks pass. Final full-window playback,
live-service and release acceptance remain pending.
