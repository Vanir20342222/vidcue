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

Implementation awaits exact-source native qualification. The independent judge approved a two-by-two
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
The corrected candidate backs their wrapper views and explicitly tests those
controls without erasing glyphs or changing seek amounts. A fixed dark timeline
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
