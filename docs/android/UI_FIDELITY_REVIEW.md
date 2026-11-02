# M9: judged Android UI and UX refinement

On 2026-11-01 the owner requested continued comparison and refinement toward
the supplied YouTube references, with an independent judge for every revision.
The baseline is mobile.10 app source `686e379`, main documentation `2baa9b6`.
Work keeps ytui branding, the YouTube tab label, all themes/player styles/actions,
saved private state and existing playback/network behavior.

## Evidence boundary

Only `ytui-youtube-restyle-agent-prompt.md` and `youtube-style-and-feel.md` were
attached. The original YouTube screenshot ZIP and the 64 source reference images
are still absent. Retained ytui screenshots qualify the current app; they are
not substitute YouTube references. The judge can assess the written requirements
and concrete usability defects, but cannot certify a pixel-identical replica.
Physical touch/OS behavior and live services remain separate gates.

## Independent baseline decision: changes required

The read-only judge inspected full-size actual native captures and source and
reported these actionable findings:

1. At 2x text, bottom-navigation words split inside words and lose alignment.
2. History rows and its destructive action look like a stack of primary pills.
3. An empty queue reserves almost the entire sheet through its weighted list.
4. Timer choices are primary-pill stacks instead of neutral action rows.
5. Nested Settings has competing Back actions and a repeated section heading.
6. Active search retains the redundant brand/action header above its search bar.
7. Account repeats its title with a marketing heading and oversized hero icon.
8. Wide search omits the existing thumbnail-duration badge.
9. Native transport glyphs need local contrast on bright video.
10. Shuffle/repeat do not expose their real selected state visually or to readers.

Supplemental evidence must include populated queue/history, mini player, tablet
side/focused playback, compact empty queue, large-text navigation and transitions.

## Candidate revision 1

Native qualification failed; this candidate is not ready to release. The independent judge approved a two-by-two
fully scaled phone navigation adaptation as an explicit accessibility exception,
preserving complete words and all four visible targets. This is not presented as
a literal match to unavailable YouTube screenshots.

Navigation measures the actual full-size bold labels to choose a normal bar or
two rows. Cells place icon/label horizontally when space permits, otherwise stack
them consistently. The tablet rail expands to fit the same words. A single
inset-aware width decision selects the rail/footer, avoiding a breakpoint gap
where window width and usable content width disagreed. Both phone and tablet
navigation receive real 2x text-layout/overflow checks.

Existing command handlers and native playback lifecycle are retained. To display
real shuffle/repeat selection, the Android status snapshot adds two booleans from
existing Media3 getters (`shuffle` and `repeat_one`). This additive presentation
contract is documented explicitly: command implementations, Rust models,
extraction, network, cache and queue-selection policy do not change.

The candidate replaces pill stacks with neutral artwork/action rows, keeps real
queue reorder/remove handlers, fits empty queues to content, consolidates nested
Settings navigation, and removes the Account marketing hierarchy. Browse moves
into More while retaining its description/tag, leaving three app-bar actions.
Focused/submitted searches use one compact search header and preserve the query
on Back; existing search/back repository actions remain the handlers. Wide
thumbnails retain the same duration/live badge and visibility preference.

The source judge also rejected the initial contrast probe: Media3's actual seek
controls use `*_with_amount` IDs and their arrow graphics are Button backgrounds.
Revision 1 backed their apparent wrapper views and tested those controls; the
actual-capture judge subsequently found an API-qualified layout regression,
recorded below. Seek amounts remain unchanged. A fixed dark timeline
backing also failed light palettes; the narrow keyline now chooses black/white
from the real accent. The native test requires actual visible white video before
measuring icon/track contrast in both built-in light/dark palettes. These are
candidate-review findings, not bugs claimed to have shipped in mobile.10.

Native evidence adds populated queue/history, compact empty queue, timer/feeds,
mini player, selected modes in focused/tablet players, hardware/soft-IME search
Back, and full-word 2x navigation. A synthetic-media interaction recording runs
with animations enabled and the Compose clock advanced alongside wall time;
it checks that player expansion/minimization and sheets retain the decoder and
ongoing playback. Reduced-motion behavior and all original startup limits remain.

Local candidate checks pass: debug APK, instrumentation APK, 21 JVM tests and
lint. An unused inner constraint scope initially failed lint after the shared
breakpoint calculation was moved outward; the redundant wrapper was removed,
with no suppression or baseline change. Native checks and the screenshot/motion
judge decision remain pending and will identify the exact qualified revision.

Each candidate must identify its exact source and native artifacts, retain failed
results, and receive another independent screenshot/interaction review. A compile
or an automated score is not visual acceptance. No revision is described as
perfect while reference, device or interaction evidence is missing.

## Judge acceptance rubric

- Evidence identifies source, dimensions, theme/font scale and settled decoded UI.
- Geometry preserves full-bleed 16:9 video, square music art, text insets and
  48dp targets without overlap, clipping or broken navigation words at 2x.
- Hierarchy uses quiet list rows and content-sized empty sheets, with one clear
  primary action and unambiguous Back/Close destinations.
- Body contrast reaches 4.5:1 and icon contrast 3:1. Modes expose real selected
  appearance and semantics, with usable descriptions and unchanged tags/actions.
- Phone/tablet, portrait/landscape, normal/2x text, existing themes and all player
  styles are covered. Native gestures, focus/IME, queue commands, collapse and
  fullscreen/return remain qualified. Video evidence is required for motion.
- No high/medium finding remains within the judged scope; missing original
  reference images and physical/live behavior stay explicitly unverified.

## Revision 1 retained native result and revision 2 candidate

Source `d70291c47512c03eb9ea919b27ffcf6068fb1c0c` (workflow merge
`eee39677dd290ac062b0f7c8c47213e8ad02f593`) ran in
[Android 41357033289](https://github.com/Vanir20342222/ytui/actions/runs/41357033289):
**36 native passes, six failures, one explicit public-YouTube skip**. Its
[device artifact](https://github.com/Vanir20342222/ytui/actions/runs/41357033289/artifacts/13104371956)
retains 212 PNGs, including all 156 theme/system-mode/device/screen captures,
and an actual interaction MP4. Rust/media checks pass (367 passes, 31 explicit
ignores), as do secret scans. Progressive cold/prepared actual-window checks are
998/992 ms, indexed 1342 ms, HLS cold/prepared 681/729 ms. Those fixture timings
do not qualify public YouTube or visual perfection.

The failures remain recorded rather than waived:

- Large-text navigation reports overflow before the original capture point.
  The next candidate measures one explicitly resolved label style for measurement and drawing, including
  letter spacing and rounding space. Cell minimum heights also derive from the
  measured full-scale label height, preserving icon/gap/padding room. It captures
  the screen before asserting and reports text, constraints and overflow axis.
- Both search Back checks time out waiting for the ordinary header. The next
  candidate separates soft-keyboard dismissal from leaving search, reads actual
  platform IME visibility at Back dispatch, and preserves the query. Drivers
  exercise explicit exit and platform Back with/without a visible IME.
- A feed artwork pixel check times out when the same media also appears in the
  retained player dock. Its matcher is scoped to the feed card; decoded-pixel
  requirements and the deadline remain unchanged.
- The white-video probe waits before the Compose-hosted surface is driven.
  The corrected driver uses Main effects and waits for that surface first,
  retaining actual-white-pixel, native control and contrast assertions.
- The motion recording exists and decodes externally, but Android's metadata
  service cannot reopen its app-scoped path. The driver passes an open file
  descriptor instead. The real interaction/decoder assertions had already
  passed; the duration/sidecar assertions must pass in the next native run.

These are revision 2 candidates pending local checks, full native rerun and
independent visual review. No assertion, 2000 ms startup limit, lint rule or
secret check is disabled to obtain a passing result.

The independent revision 1 visual judge found a high-priority regression in
`native-video-controls` and fullscreen: a dark ellipse covered the entire middle
transport strip. The initial source audit had examined the legacy Media3 seek
layout; API 23+ uses a merge layout with arrows in each Button's foreground.
Backing the parent therefore painted the common controls container. Revision 2
backs each actual Button, retaining its foreground/amount (or original layered
background glyph), and explicitly rejects an oval on the common parent. Bright
video measurements and full-strip captures must qualify the correction.

The judge accepted the quieter history/timer/feed/account/settings/search
hierarchy in the retained captures. It requested a denser queue: revision 2 uses
one artwork/title row plus a trailing overflow containing the same Play, Move up,
Move down and Remove commands. Existing action tags/descriptions and boundary
states are retained. Drivers now open the intended menu before checking the
same real Rust reorder/remove outcomes and capture its expanded state. This is
a deliberate presentation change to the previous always-visible action strip,
not a relaxed queue assertion.

The motion judge sampled 50 frames from the actual 12.52-second revision 1 clip:
player expansion, swipe/chevron minimization and timer opening showed no obvious
blank frame/bounce, but the clip ended before dismissal. Revision 2 reserves a
30-second recording budget, asserts the timer is absent while screenrecord is
still running, retains a settled interval after that state, and writes action
timestamps via the file-descriptor metadata path. The longer recording wait is
an evidence-collection timeout; all media-start deadlines remain 2000 ms.
The cloud recording's variable frame rate is not evidence of physical 60fps.

Local revision 2 application checks pass: debug APK, instrumentation APK,
21 JVM tests, lint. The final recording-only change is undergoing the same local
checks before its full native CI run. All-history redacted scanning finds no
leaks in 162 commits; new commits also receive CI secret checks.
