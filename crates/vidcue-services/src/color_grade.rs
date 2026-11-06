//! Colour grading for video playback: fixed looks plus an optional auto grade.
//!
//! Grades use mpv's video equalizer properties (brightness, contrast, saturation,
//! gamma; applied by the VO, so they cost nothing on the CPU). Warmth uses a small
//! labelled FFmpeg `colorbalance` filter, added only when it is non-zero.
//!
//! Auto grade measures the *ungraded* picture: for a short window it prepends a
//! labelled `signalstats` probe to the filter chain (ahead of the warmth filter;
//! the equalizer runs after filters), reads the per-frame statistics through
//! `vf-metadata`, removes the probe again and derives a slight correction. It
//! looks again periodically and blends new results in so changes stay gradual.
//! With grading off and auto grade off nothing is sent to mpv.
use serde_json::{Value, json};
use std::time::Duration;
use tokio::time::Instant;

const PROBE_LABEL: &str = "@vidcue-grade-probe";
const PROBE: &str = "@vidcue-grade-probe:lavfi=[signalstats]";
/// mpv property holding the probe's latest frame statistics.
pub const PROBE_METADATA: &str = "vf-metadata/vidcue-grade-probe";
const FILTER_LABEL: &str = "@vidcue-grade";
/// Wait after playback starts before the first look, so fade-ins and black
/// intro frames do not decide the grade.
const FIRST_LOOK: Duration = Duration::from_secs(3);
const INTERVAL: Duration = Duration::from_secs(30);
const RETRY: Duration = Duration::from_secs(60);
const WINDOW: Duration = Duration::from_millis(2500);
const SAMPLES: usize = 6;
/// Largest total adjustment per property; presets plus auto stay subtle.
const LIMIT: f64 = 30.0;

/// Adjustments on mpv's -100..100 equalizer scale; `temperature` > 0 is warmer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Grade {
    pub brightness: f64,
    pub contrast: f64,
    pub saturation: f64,
    pub gamma: f64,
    pub temperature: f64,
}
impl Grade {
    fn values(self) -> [f64; 5] {
        [
            self.brightness,
            self.contrast,
            self.saturation,
            self.gamma,
            self.temperature,
        ]
    }
    fn from_values(v: [f64; 5]) -> Self {
        Self {
            brightness: v[0],
            contrast: v[1],
            saturation: v[2],
            gamma: v[3],
            temperature: v[4],
        }
    }
    fn combine(self, other: Self, f: impl Fn(f64, f64) -> f64) -> Self {
        let (a, b) = (self.values(), other.values());
        Self::from_values(std::array::from_fn(|i| f(a[i], b[i])))
    }
    fn rounded(self) -> [i64; 5] {
        self.values().map(|v| v.clamp(-LIMIT, LIMIT).round() as i64)
    }
}

/// The fixed look for a `color_grade` setting; unknown names and "off" are neutral.
pub fn preset(name: &str) -> Grade {
    let (brightness, contrast, saturation, gamma, temperature) = match name {
        "warm" => (0.0, 0.0, 3.0, 0.0, 10.0),
        "cool" => (0.0, 2.0, 0.0, 0.0, -10.0),
        "vivid" => (0.0, 5.0, 15.0, 0.0, 0.0),
        "cinematic" => (0.0, 8.0, -10.0, -3.0, 4.0),
        _ => return Grade::default(),
    };
    Grade {
        brightness,
        contrast,
        saturation,
        gamma,
        temperature,
    }
}

/// One frame's picture statistics, normalised to 0..1 (luma) or about -1..1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    /// Average luma.
    pub mean: f64,
    /// 10th and 90th luma percentiles.
    pub low: f64,
    pub high: f64,
    /// Average chroma distance from neutral grey.
    pub saturation: f64,
    /// Red-minus-blue chroma balance; positive is a warm cast.
    pub cast: f64,
}

/// Bits per component of an mpv `video-params/pixelformat` name such as
/// `yuv420p`, `yuv420p10` or `p010`; None when the name gives no depth.
pub fn pixel_depth(format: &str) -> Option<f64> {
    let f = format.trim_end_matches("le").trim_end_matches("be");
    let digits = |s: &str| s.parse::<f64>().ok().filter(|d| (8.0..=16.0).contains(d));
    if let Some(rest) = f.strip_prefix('p').filter(|r| r.len() == 3) {
        return digits(rest);
    }
    if let Some(rest) = f.strip_prefix("gray") {
        return if rest.is_empty() {
            Some(8.0)
        } else {
            digits(rest)
        };
    }
    if f.starts_with("yuv") || f.starts_with("gbr") {
        let tail = &f[f.rfind('p')? + 1..];
        return if tail.is_empty() {
            Some(8.0)
        } else {
            digits(tail)
        };
    }
    matches!(f, "nv12" | "nv21" | "rgb24" | "bgr24" | "bgr0" | "rgb0").then_some(8.0)
}

/// Reads FFmpeg `signalstats` frame metadata. `depth` is the decoded bit depth
/// from [`pixel_depth`]; some FFmpeg builds report 8 in `YBITDEPTH` even for
/// 10-bit frames. Returns None for missing data and near-black frames, which
/// say nothing about the grade.
pub fn stats(metadata: &Value, depth: Option<f64>) -> Option<Stats> {
    let get = |key: &str| {
        let v = &metadata[format!("lavfi.signalstats.{key}")];
        v.as_str()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .or_else(|| v.as_f64())
            .filter(|v| v.is_finite())
    };
    let reported = get("YBITDEPTH").unwrap_or(8.0);
    let depth = match depth {
        Some(depth) => depth.max(reported),
        None if reported <= 8.0 && get("YMAX").unwrap_or(0.0) > 255.0 => 10.0,
        None => reported,
    };
    // Scale back to 8-bit code values; video is assumed limited range (16-235).
    let scale = 2f64.powf(depth.clamp(8.0, 16.0) - 8.0);
    let luma = |key: &str| get(key).map(|v| ((v / scale - 16.0) / 219.0).clamp(0.0, 1.0));
    let chroma = |key: &str| get(key).map(|v| v / scale);
    let stats = Stats {
        mean: luma("YAVG")?,
        low: luma("YLOW")?,
        high: luma("YHIGH")?,
        saturation: chroma("SATAVG")? / 128.0,
        cast: (chroma("VAVG")? - chroma("UAVG")?) / 128.0,
    };
    (stats.high >= 0.06).then_some(stats)
}

/// A slight correction that moves the picture towards a balanced exposure,
/// contrast, saturation and white balance. Every term is capped to a few points.
pub fn auto(s: &Stats) -> Grade {
    let spread = s.high - s.low;
    Grade {
        // Lifted, washed-out blacks come down a little.
        brightness: (-(s.low - 0.15) * 20.0).clamp(-4.0, 0.0),
        contrast: ((0.6 - spread) * 25.0).clamp(-3.0, 8.0),
        saturation: ((0.2 - s.saturation) * 50.0).clamp(-5.0, 8.0),
        // Positive gamma brightens midtones in mpv.
        gamma: ((0.42 - s.mean) * 30.0).clamp(-5.0, 8.0),
        // Neutralise part of a colour cast, never all of it.
        temperature: (-s.cast * 80.0).clamp(-6.0, 6.0),
    }
}

fn average(samples: &[Stats]) -> Stats {
    let n = samples.len() as f64;
    let sum = |f: fn(&Stats) -> f64| samples.iter().map(f).sum::<f64>() / n;
    Stats {
        mean: sum(|s| s.mean),
        low: sum(|s| s.low),
        high: sum(|s| s.high),
        saturation: sum(|s| s.saturation),
        cast: sum(|s| s.cast),
    }
}

fn warmth_filter(temperature: i64) -> String {
    let t = temperature as f64;
    format!(
        "{FILTER_LABEL}:lavfi=[colorbalance=rm={:.3}:bm={:.3}:rh={:.3}:bh={:.3}]",
        t * 0.004,
        -t * 0.004,
        t * 0.002,
        -t * 0.002
    )
}

/// What the player needs to know each poll.
pub struct Input<'a> {
    pub preset: &'a str,
    pub auto: bool,
    /// Normal video playback (not audio-only).
    pub video: bool,
    /// Frames are advancing: not paused, not buffering, position known.
    pub playing: bool,
    /// The [`PROBE_METADATA`] property value.
    pub metadata: &'a Value,
    /// mpv `video-params/pixelformat`, for the probe's bit depth.
    pub pixel_format: Option<&'a str>,
    pub now: Instant,
}

/// Per-mpv-window grading state.
#[derive(Default)]
pub struct State {
    applied: Option<[i64; 5]>,
    auto: Option<Grade>,
    probe_since: Option<Instant>,
    samples: Vec<Stats>,
    next_look: Option<Instant>,
}
impl State {
    /// The auto correction currently in effect, if one has been measured.
    pub fn auto_grade(&self) -> Option<Grade> {
        self.auto
    }
    /// mpv IPC commands that bring the window to the wanted grade. Empty when
    /// nothing changed.
    pub fn commands(&mut self, input: Input<'_>) -> Vec<Value> {
        let mut out = Vec::new();
        if !(input.auto && input.video) {
            if self.probe_since.take().is_some() {
                out.push(json!(["vf", "remove", PROBE_LABEL]));
            }
            self.samples.clear();
            self.auto = None;
            self.next_look = None;
        } else if let Some(since) = self.probe_since {
            if let Some(s) = stats(input.metadata, input.pixel_format.and_then(pixel_depth))
                && self.samples.last() != Some(&s)
            {
                self.samples.push(s);
            }
            if self.samples.len() >= SAMPLES
                || input.now.duration_since(since) >= WINDOW
                || !input.playing
            {
                out.push(json!(["vf", "remove", PROBE_LABEL]));
                self.probe_since = None;
                if !self.samples.is_empty() {
                    let target = auto(&average(&self.samples));
                    self.auto = Some(match self.auto {
                        Some(previous) => previous.combine(target, |a, b| a + (b - a) * 0.5),
                        None => target,
                    });
                }
                let wait = if self.samples.is_empty() && self.auto.is_none() {
                    RETRY
                } else {
                    INTERVAL
                };
                self.samples.clear();
                self.next_look = Some(input.now + wait);
            }
        } else if input.playing {
            let due = *self.next_look.get_or_insert(input.now + FIRST_LOOK);
            if input.now >= due {
                out.push(json!(["vf", "pre", PROBE]));
                self.probe_since = Some(input.now);
            }
        }
        let wanted = if input.video {
            preset(input.preset)
                .combine(self.auto.unwrap_or_default(), |a, b| a + b)
                .rounded()
        } else {
            [0; 5]
        };
        if self.applied == Some(wanted) || (self.applied.is_none() && wanted == [0; 5]) {
            return out;
        }
        let previous = self.applied.unwrap_or([0; 5]);
        for (i, name) in ["brightness", "contrast", "saturation", "gamma"]
            .into_iter()
            .enumerate()
        {
            if previous[i] != wanted[i] {
                out.push(json!(["set_property", name, wanted[i]]));
            }
        }
        if previous[4] != wanted[4] {
            if previous[4] != 0 {
                out.push(json!(["vf", "remove", FILTER_LABEL]));
            }
            if wanted[4] != 0 {
                out.push(json!(["vf", "add", warmth_filter(wanted[4])]));
            }
        }
        self.applied = Some(wanted);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(yavg: f64, ylow: f64, yhigh: f64, sat: f64, u: f64, v: f64) -> Value {
        json!({
            "lavfi.signalstats.YAVG": yavg.to_string(),
            "lavfi.signalstats.YLOW": ylow.to_string(),
            "lavfi.signalstats.YHIGH": yhigh.to_string(),
            "lavfi.signalstats.YMAX": "235",
            "lavfi.signalstats.SATAVG": sat.to_string(),
            "lavfi.signalstats.UAVG": u.to_string(),
            "lavfi.signalstats.VAVG": v.to_string(),
        })
    }
    fn input<'a>(preset: &'a str, auto: bool, metadata: &'a Value, now: Instant) -> Input<'a> {
        Input {
            preset,
            auto,
            video: true,
            playing: true,
            metadata,
            pixel_format: Some("yuv420p"),
            now,
        }
    }

    #[test]
    fn off_sends_nothing_and_presets_are_applied_once() {
        let none = Value::Null;
        let now = Instant::now();
        let mut state = State::default();
        assert!(state.commands(input("off", false, &none, now)).is_empty());
        let commands = state.commands(input("warm", false, &none, now));
        assert!(commands.contains(&json!(["set_property", "saturation", 3])));
        assert!(
            commands.iter().any(|c| c[0] == "vf"
                && c[1] == "add"
                && c[2].as_str().unwrap().contains("rm=0.040"))
        );
        assert!(state.commands(input("warm", false, &none, now)).is_empty());
        // Switching back to off resets every property and removes the warmth filter.
        let reset = state.commands(input("off", false, &none, now));
        assert!(reset.contains(&json!(["set_property", "saturation", 0])));
        assert!(reset.contains(&json!(["vf", "remove", FILTER_LABEL])));
        assert!(!reset.iter().any(|c| c[1] == "add"));
    }

    #[test]
    fn audio_playback_is_never_graded() {
        let none = Value::Null;
        let mut state = State::default();
        let mut i = input("vivid", true, &none, Instant::now());
        i.video = false;
        assert!(state.commands(i).is_empty());
    }

    #[test]
    fn signalstats_are_normalised_for_8_and_10_bit_video() {
        let s = stats(&frame(125.5, 50.0, 200.0, 32.0, 120.0, 136.0), Some(8.0)).unwrap();
        assert!((s.mean - 0.5).abs() < 1e-9);
        assert!((s.saturation - 0.25).abs() < 1e-9);
        assert!((s.cast - 0.125).abs() < 1e-9);
        // FFmpeg 7 reports YBITDEPTH=8 for 10-bit frames; the pixel format decides.
        let mut ten = frame(502.0, 200.0, 800.0, 128.0, 480.0, 544.0);
        ten["lavfi.signalstats.YBITDEPTH"] = json!("8");
        ten["lavfi.signalstats.YMAX"] = json!("940");
        let t = stats(&ten, pixel_depth("yuv420p10")).unwrap();
        assert!((t.mean - 0.5).abs() < 1e-9 && (t.cast - 0.125).abs() < 1e-9);
        // A dark 10-bit frame (YMAX below 256) is not mistaken for a bright 8-bit one.
        let mut dark = frame(100.0, 70.0, 140.0, 20.0, 512.0, 512.0);
        dark["lavfi.signalstats.YMAX"] = json!("200");
        assert!(stats(&dark, pixel_depth("p010")).unwrap().mean < 0.05 + 1e-9);
        // Without a format hint, values above 8-bit range imply 10-bit.
        assert!((stats(&ten, None).unwrap().mean - 0.5).abs() < 1e-9);
        assert!(
            stats(&frame(16.0, 16.0, 18.0, 0.0, 128.0, 128.0), Some(8.0)).is_none(),
            "black frame"
        );
        assert!(stats(&Value::Null, None).is_none());
    }

    #[test]
    fn pixel_formats_give_their_depth() {
        assert_eq!(pixel_depth("yuv420p"), Some(8.0));
        assert_eq!(pixel_depth("yuv420p10"), Some(10.0));
        assert_eq!(pixel_depth("yuv444p12le"), Some(12.0));
        assert_eq!(pixel_depth("p010"), Some(10.0));
        assert_eq!(pixel_depth("nv12"), Some(8.0));
        assert_eq!(pixel_depth("gray10"), Some(10.0));
        assert_eq!(pixel_depth("vaapi"), None);
    }

    #[test]
    fn auto_grade_is_slight_and_points_the_right_way() {
        let dark_flat_warm = Stats {
            mean: 0.2,
            low: 0.1,
            high: 0.35,
            saturation: 0.08,
            cast: 0.1,
        };
        let g = auto(&dark_flat_warm);
        assert!(g.gamma > 0.0 && g.contrast > 0.0 && g.saturation > 0.0);
        assert!(g.temperature < 0.0, "a warm cast is cooled a little");
        let bright_vivid = Stats {
            mean: 0.7,
            low: 0.3,
            high: 0.95,
            saturation: 0.45,
            cast: -0.1,
        };
        let g = auto(&bright_vivid);
        assert!(g.gamma < 0.0 && g.saturation < 0.0 && g.brightness < 0.0);
        assert!(g.temperature > 0.0);
        let balanced = Stats {
            mean: 0.42,
            low: 0.12,
            high: 0.72,
            saturation: 0.2,
            cast: 0.0,
        };
        assert_eq!(auto(&balanced).rounded(), [0; 5]);
        for s in [dark_flat_warm, bright_vivid] {
            assert!(auto(&s).values().iter().all(|v| v.abs() <= 8.0));
        }
    }

    #[test]
    fn auto_grade_probes_after_a_delay_then_removes_the_probe_and_applies() {
        let none = Value::Null;
        let dark = frame(60.0, 30.0, 110.0, 10.0, 128.0, 128.0);
        let start = Instant::now();
        let mut state = State::default();
        assert!(state.commands(input("off", true, &none, start)).is_empty());
        let probe_at = start + FIRST_LOOK;
        let add = state.commands(input("off", true, &none, probe_at));
        assert_eq!(add, vec![json!(["vf", "pre", PROBE])]);
        // Collect a few frames, then the window closes and the grade is applied.
        let mut commands = Vec::new();
        for i in 0..SAMPLES {
            let mut f = dark.clone();
            f["lavfi.signalstats.YAVG"] = json!(format!("{}", 60 + i));
            commands = state.commands(input(
                "off",
                true,
                &f,
                probe_at + Duration::from_millis(125 * (i as u64 + 1)),
            ));
        }
        assert_eq!(commands[0], json!(["vf", "remove", PROBE_LABEL]));
        assert!(
            commands
                .iter()
                .any(|c| c[0] == "set_property" && c[1] == "gamma" && c[2].as_i64() > Some(0))
        );
        let g = state.auto_grade().unwrap();
        assert!(g.gamma > 0.0);
        // No new probe until the interval passes.
        let later = probe_at + Duration::from_secs(5);
        assert!(state.commands(input("off", true, &none, later)).is_empty());
        let again = state.commands(input(
            "off",
            true,
            &none,
            probe_at + INTERVAL + Duration::from_secs(2),
        ));
        assert_eq!(again, vec![json!(["vf", "pre", PROBE])]);
        // Turning auto off removes the probe and the correction.
        let off = state.commands(input("off", false, &none, later + INTERVAL));
        assert_eq!(off[0], json!(["vf", "remove", PROBE_LABEL]));
        assert!(off.contains(&json!(["set_property", "gamma", 0])));
        assert!(state.auto_grade().is_none());
    }

    #[test]
    fn pausing_closes_the_probe_and_no_frames_means_a_later_retry() {
        let none = Value::Null;
        let start = Instant::now();
        let mut state = State::default();
        state.commands(input("off", true, &none, start));
        state.commands(input("off", true, &none, start + FIRST_LOOK));
        let mut paused = input(
            "off",
            true,
            &none,
            start + FIRST_LOOK + Duration::from_secs(1),
        );
        paused.playing = false;
        assert_eq!(
            state.commands(paused),
            vec![json!(["vf", "remove", PROBE_LABEL])]
        );
        assert!(state.auto_grade().is_none());
        let soon = start + FIRST_LOOK + INTERVAL + Duration::from_secs(2);
        assert!(state.commands(input("off", true, &none, soon)).is_empty());
        let retry = start + FIRST_LOOK + Duration::from_secs(1) + RETRY;
        assert_eq!(
            state.commands(input("off", true, &none, retry)),
            vec![json!(["vf", "pre", PROBE])]
        );
    }

    #[test]
    fn presets_combine_with_auto_within_limits() {
        let mut big = preset("vivid");
        big.saturation = 100.0;
        assert_eq!(big.rounded()[2], LIMIT as i64);
        assert_eq!(preset("off"), Grade::default());
        assert_eq!(preset("unknown"), Grade::default());
        for name in vidcue_core::COLOR_GRADES {
            assert!(preset(name).values().iter().all(|v| v.abs() <= 15.0));
        }
    }
}
