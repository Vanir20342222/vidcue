//! Ten-band audio equalizer: built-in presets, user-named custom presets and the
//! mpv/FFmpeg filter that applies them. "off" adds no filter at all.
use anyhow::{Result, ensure};
use std::collections::BTreeMap;

/// Centre frequencies (Hz) of the ten octave bands.
pub const BANDS: [u32; 10] = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
/// Largest boost or cut per band, in dB.
pub const MAX_GAIN: f64 = 12.0;
/// Most user-named presets kept in settings.
pub const MAX_CUSTOM: usize = 16;
/// The preset name that disables the equalizer.
pub const OFF: &str = "off";
/// The name setup uses for a new custom curve.
pub const CUSTOM: &str = "custom";

/// Built-in presets in display order. Gains are dB per band in [`BANDS`] order.
pub const PRESETS: &[(&str, [f64; 10])] = &[
    ("flat", [0.0; 10]),
    (
        "bass-boost",
        [6.0, 5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ),
    (
        "bass-reduce",
        [-6.0, -5.0, -4.0, -2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ),
    (
        "treble-boost",
        [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 5.0, 6.0],
    ),
    (
        "vocal",
        [-2.0, -2.0, -1.0, 1.0, 3.0, 4.0, 3.0, 1.0, 0.0, -1.0],
    ),
    (
        "rock",
        [5.0, 4.0, 2.0, -1.0, -2.0, -1.0, 1.0, 3.0, 4.0, 4.0],
    ),
    ("pop", [-1.0, 0.0, 2.0, 3.0, 4.0, 3.0, 1.0, 0.0, -1.0, -1.0]),
    ("jazz", [3.0, 2.0, 1.0, 2.0, -1.0, -1.0, 0.0, 1.0, 2.0, 3.0]),
    (
        "classical",
        [4.0, 3.0, 2.0, 1.0, -1.0, -1.0, 0.0, 2.0, 3.0, 4.0],
    ),
    (
        "electronic",
        [5.0, 4.0, 1.0, 0.0, -2.0, 1.0, 0.0, 1.0, 4.0, 5.0],
    ),
    (
        "acoustic",
        [3.0, 3.0, 2.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 1.0],
    ),
    (
        "loudness",
        [6.0, 4.0, 0.0, 0.0, -2.0, 0.0, -1.0, -3.0, 4.0, 2.0],
    ),
    (
        "spoken-word",
        [-4.0, -3.0, -1.0, 1.0, 3.0, 4.0, 4.0, 2.0, 0.0, -3.0],
    ),
];

/// Every preset name the user can choose: off, the built-ins, then custom names.
pub fn names(custom: &BTreeMap<String, Vec<f64>>) -> Vec<String> {
    std::iter::once(OFF)
        .chain(PRESETS.iter().map(|(name, _)| *name))
        .map(str::to_owned)
        .chain(custom.keys().cloned())
        .collect()
}

pub fn is_builtin(name: &str) -> bool {
    name == OFF || PRESETS.iter().any(|(n, _)| *n == name)
}

/// Gains for `name`, or `None` for "off" and unknown names.
pub fn gains(name: &str, custom: &BTreeMap<String, Vec<f64>>) -> Option<[f64; 10]> {
    if let Some((_, gains)) = PRESETS.iter().find(|(n, _)| *n == name) {
        return Some(*gains);
    }
    let values = custom.get(name)?;
    let mut gains = [0.0; 10];
    for (slot, value) in gains.iter_mut().zip(values) {
        *slot = *value;
    }
    Some(gains)
}

/// The mpv `--af` value for a curve, labelled `@vidcue-eq` so it can be replaced
/// live. A curve with every band at 0 dB needs no filter.
pub fn filter(gains: &[f64; 10]) -> Option<String> {
    if gains.iter().all(|g| *g == 0.0) {
        return None;
    }
    let chain = BANDS
        .iter()
        .zip(gains)
        .filter(|(_, g)| **g != 0.0)
        .map(|(f, g)| format!("equalizer=f={f}:t=o:w=1:g={g}"))
        .collect::<Vec<_>>()
        .join(",");
    Some(format!("@vidcue-eq:lavfi=[{chain}]"))
}

/// One peaking band, the same RBJ design FFmpeg's `equalizer` filter uses with an
/// octave width of 1, so Android sounds like desktop mpv.
#[derive(Clone, Copy, Debug)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}
impl Biquad {
    fn peaking(frequency: f64, gain: f64, sample_rate: f64) -> Self {
        let w0 = std::f64::consts::TAU * frequency / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin * (std::f64::consts::LN_2 / 2.0 * w0 / sin).sinh();
        let a = 10f64.powf(gain / 40.0);
        let a0 = 1.0 + alpha / a;
        Self {
            b0: ((1.0 + alpha * a) / a0) as f32,
            b1: (-2.0 * cos / a0) as f32,
            b2: ((1.0 - alpha * a) / a0) as f32,
            a1: (-2.0 * cos / a0) as f32,
            a2: ((1.0 - alpha / a) / a0) as f32,
        }
    }
}

/// A ten-band filter bank for interleaved PCM with per-channel state.
#[derive(Clone, Debug)]
pub struct Bank {
    filters: Vec<Biquad>,
    /// Transposed direct form II state: two values per filter per channel.
    state: Vec<[f32; 2]>,
    channels: usize,
    /// Linear pre-gain that leaves headroom for the largest boost, so 16-bit output
    /// does not hard-clip.
    preamp: f32,
}
impl Bank {
    /// Returns `None` for a flat curve or an unusable format.
    pub fn new(gains: &[f64; 10], sample_rate: u32, channels: usize) -> Option<Self> {
        if channels == 0 || sample_rate == 0 || gains.iter().all(|g| *g == 0.0) {
            return None;
        }
        let rate = f64::from(sample_rate);
        let filters = BANDS
            .iter()
            .zip(gains)
            // Bands at or above Nyquist cannot be represented at this rate.
            .filter(|(f, g)| **g != 0.0 && f64::from(**f) < rate * 0.45)
            .map(|(f, g)| Biquad::peaking(f64::from(*f), g.clamp(-MAX_GAIN, MAX_GAIN), rate))
            .collect::<Vec<_>>();
        if filters.is_empty() {
            return None;
        }
        let boost = gains.iter().copied().fold(0.0, f64::max).min(MAX_GAIN);
        Some(Self {
            state: vec![[0.0; 2]; filters.len() * channels],
            filters,
            channels,
            preamp: 10f64.powf(-boost / 20.0) as f32,
        })
    }
    /// Clears filter memory, e.g. after a seek.
    pub fn reset(&mut self) {
        self.state.iter_mut().for_each(|s| *s = [0.0; 2]);
    }
    /// Filters interleaved little-endian signed 16-bit samples in place.
    pub fn process_pcm16(&mut self, bytes: &mut [u8]) {
        let bands = self.filters.len();
        for (index, sample) in bytes.as_chunks_mut::<2>().0.iter_mut().enumerate() {
            let channel = index % self.channels;
            let mut x = f32::from(i16::from_le_bytes(*sample)) * self.preamp;
            for (filter, state) in self
                .filters
                .iter()
                .zip(&mut self.state[channel * bands..(channel + 1) * bands])
            {
                let y = filter.b0 * x + state[0];
                state[0] = filter.b1 * x - filter.a1 * y + state[1];
                state[1] = filter.b2 * x - filter.a2 * y;
                x = y;
            }
            let out = x.round().clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;
            *sample = out.to_le_bytes();
        }
    }
}

/// Validates a preset name and the custom table.
pub fn validate(name: &str, custom: &BTreeMap<String, Vec<f64>>) -> Result<()> {
    ensure!(
        custom.len() <= MAX_CUSTOM,
        "At most {MAX_CUSTOM} custom equalizer presets are allowed"
    );
    for (key, values) in custom {
        ensure!(
            valid_name(key) && !is_builtin(key),
            "Custom equalizer names use letters, digits and '-', up to 24 characters, and cannot reuse a built-in name ({key})"
        );
        ensure!(
            values.len() == BANDS.len(),
            "Custom equalizer {key} needs {} band values",
            BANDS.len()
        );
        ensure!(
            values.iter().all(|g| g.is_finite() && g.abs() <= MAX_GAIN),
            "Equalizer gains must be between -{MAX_GAIN} and {MAX_GAIN} dB"
        );
    }
    ensure!(
        is_builtin(name) || custom.contains_key(name),
        "Unknown equalizer preset {name}"
    );
    Ok(())
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 24
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Settings text form: `name=g1,g2,…,g10; other=…`. Names are lower-cased.
pub fn format_custom(custom: &BTreeMap<String, Vec<f64>>) -> String {
    custom
        .iter()
        .map(|(name, gains)| {
            let gains = gains.iter().map(|g| g.to_string()).collect::<Vec<_>>();
            format!("{name}={}", gains.join(","))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn parse_custom(text: &str) -> Result<BTreeMap<String, Vec<f64>>> {
    let mut custom = BTreeMap::new();
    for entry in text.split(';').map(str::trim).filter(|e| !e.is_empty()) {
        let (name, gains) = entry
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("Write custom equalizers as name=g1,…,g10"))?;
        let gains = gains
            .split(',')
            .map(|g| g.trim().parse::<f64>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| anyhow::anyhow!("Equalizer gains must be numbers in dB"))?;
        let name = name.trim().to_lowercase();
        ensure!(
            custom.insert(name.clone(), gains).is_none(),
            "Custom equalizer {name} is listed twice"
        );
    }
    Ok(custom)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tone(frequency: f64, rate: u32, seconds: f64, amplitude: f64) -> Vec<u8> {
        (0..(f64::from(rate) * seconds) as usize)
            .flat_map(|n| {
                let v = (amplitude
                    * (std::f64::consts::TAU * frequency * n as f64 / f64::from(rate)).sin())
                    as i16;
                // Stereo: the same sample on both channels.
                [v.to_le_bytes(), v.to_le_bytes()].concat()
            })
            .collect()
    }
    fn rms_db(bytes: &[u8]) -> f64 {
        // Skip the first half so the filters have settled.
        let samples = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .skip(bytes.len() / 4)
            .map(|s| f64::from(i16::from_le_bytes(*s)))
            .collect::<Vec<_>>();
        let mean = samples.iter().map(|s| s * s).sum::<f64>() / samples.len() as f64;
        10.0 * mean.log10()
    }
    #[test]
    fn filter_bank_applies_band_gains_with_headroom_and_skips_flat_curves() {
        assert!(Bank::new(&[0.0; 10], 48_000, 2).is_none());
        assert!(Bank::new(&[3.0; 10], 48_000, 0).is_none());
        let rate = 48_000;
        let mut cut = [0.0; 10];
        cut[5] = -12.0; // 1 kHz
        for (frequency, expected) in [(1000.0, -12.0), (62.0, 0.0), (8000.0, 0.0)] {
            let input = tone(frequency, rate, 0.5, 8000.0);
            let mut output = input.clone();
            let mut bank = Bank::new(&cut, rate, 2).unwrap();
            // Odd chunk sizes: state carries across buffers.
            for chunk in output.chunks_mut(1234) {
                bank.process_pcm16(chunk);
            }
            let change = rms_db(&output) - rms_db(&input);
            assert!(
                (change - expected).abs() < 0.5,
                "{frequency} Hz changed by {change} dB"
            );
        }
        // A boost is offset by the pre-gain, so the boosted band stays at its level
        // and the rest is quieter instead of clipping.
        let mut boost = [0.0; 10];
        boost[0] = 6.0;
        let input = tone(31.0, rate, 1.0, 30000.0);
        let mut output = input.clone();
        let mut bank = Bank::new(&boost, rate, 2).unwrap();
        bank.process_pcm16(&mut output);
        assert!((rms_db(&output) - rms_db(&input)).abs() < 0.6);
        bank.reset();
        // 16 kHz cannot be represented at 22.05 kHz; the band is dropped.
        let mut high = [0.0; 10];
        high[9] = 6.0;
        assert!(Bank::new(&high, 22_050, 1).is_none());
        let mut odd = vec![1u8, 2, 3];
        Bank::new(&cut, rate, 1).unwrap().process_pcm16(&mut odd);
        assert_eq!(odd[2], 3, "a trailing partial sample is left alone");
    }
    #[test]
    fn presets_are_valid_and_off_or_flat_add_no_filter() {
        let none = BTreeMap::new();
        for name in names(&none) {
            validate(&name, &none).unwrap();
        }
        assert_eq!(gains(OFF, &none), None);
        assert_eq!(filter(&gains("flat", &none).unwrap()), None);
        let bass = filter(&gains("bass-boost", &none).unwrap()).unwrap();
        assert_eq!(
            bass,
            "@vidcue-eq:lavfi=[equalizer=f=31:t=o:w=1:g=6,equalizer=f=62:t=o:w=1:g=5,equalizer=f=125:t=o:w=1:g=4,equalizer=f=250:t=o:w=1:g=2]"
        );
        assert!(
            PRESETS
                .iter()
                .all(|(_, g)| g.iter().all(|v| v.abs() <= MAX_GAIN))
        );
    }
    #[test]
    fn custom_presets_roundtrip_through_text_and_are_validated() {
        let custom =
            parse_custom(" Night = -3,-2,0,0,1,2,2,1,0,-1 ; car=1,1,1,1,1,1,1,1,1,1.5").unwrap();
        assert_eq!(custom["night"][0], -3.0);
        assert_eq!(parse_custom(&format_custom(&custom)).unwrap(), custom);
        validate("night", &custom).unwrap();
        assert_eq!(names(&custom).last().unwrap(), "night");
        assert!(
            filter(&gains("car", &custom).unwrap())
                .unwrap()
                .contains("g=1.5")
        );
        assert!(validate("missing", &custom).is_err());
        assert!(
            parse_custom("x=1,2")
                .map(|c| validate(OFF, &c))
                .unwrap()
                .is_err()
        );
        assert!(
            parse_custom("rock=0,0,0,0,0,0,0,0,0,0")
                .map(|c| validate(OFF, &c))
                .unwrap()
                .is_err()
        );
        assert!(
            parse_custom("x=13,0,0,0,0,0,0,0,0,0")
                .map(|c| validate(OFF, &c))
                .unwrap()
                .is_err()
        );
        assert!(parse_custom("a=1;a=2").is_err());
        assert!(parse_custom("a=loud").is_err());
        assert!(parse_custom("").unwrap().is_empty());
    }
}
