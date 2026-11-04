//! Allocation-free PCM16 metering of this app's decoder output only.
//!
//! `pcm16` gives stereo RMS and peak for the VU strip. `spectrum_pcm16` keeps a rolling
//! mono window of the same decoder buffers and folds its FFT into log-spaced bands for the
//! frequency-driven visualisers. Neither path records audio or opens a capture device.
use std::sync::Mutex;

/// Number of log-spaced frequency bands returned to the presentation layer.
pub const BANDS: usize = 24;
const FFT_SIZE: usize = 1024;
const HALF: usize = FFT_SIZE / 2;
const MIN_HZ: f32 = 32.0;
const MAX_HZ: f32 = 16_000.0;
/// Lifts typical per-bin power of music into the display range.
const GAIN_DB: f32 = 20.0;
/// Music's power per bin falls with frequency, so treble is lifted relative to 250 Hz.
const TILT_DB_PER_OCTAVE: f32 = 2.0;
const TILT_RANGE_DB: (f32, f32) = (-6.0, 12.0);
/// Display span, matching the stereo meter's -60..0 dB scale.
const DISPLAY_RANGE_DB: f32 = 60.0;

pub fn pcm16(bytes: &[u8], channels: usize) -> u64 {
    if channels == 0 || channels > 8 || bytes.len() > 256 * 1024 || bytes.len() < channels * 2 {
        return 0;
    }
    let mut sums = [0.0_f64; 2];
    let mut peak = 0.0_f64;
    let mut frames = 0;
    for frame in bytes.chunks_exact(channels * 2) {
        for (channel, sum) in sums.iter_mut().enumerate() {
            let index = channel.min(channels - 1) * 2;
            let value = i16::from_le_bytes([frame[index], frame[index + 1]]) as f64 / 32768.0;
            *sum += value * value;
            peak = peak.max(value.abs());
        }
        frames += 1;
    }
    let normalize = |amplitude: f64| -> u64 {
        if amplitude <= 0.0 {
            0
        } else {
            (((20.0 * amplitude.log10() + 60.0) / 60.0).clamp(0.0, 1.0) * 65535.0).round() as u64
        }
    };
    normalize((sums[0] / frames as f64).sqrt())
        | normalize((sums[1] / frames as f64).sqrt()) << 16
        | normalize(peak) << 32
}

/// Shared analyser. Created on first use, then reused for the whole playback session.
static SPECTRUM: Mutex<Option<Spectrum>> = Mutex::new(None);

/// Feeds one decoder buffer into the rolling window and writes [`BANDS`] levels in
/// `0..=65535`, low to high frequency. Malformed buffers leave the window unchanged.
pub fn spectrum_pcm16(bytes: &[u8], channels: usize, sample_rate: u32, out: &mut [i32; BANDS]) {
    let mut guard = SPECTRUM
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let spectrum = guard.get_or_insert_with(Spectrum::new);
    spectrum.push(bytes, channels);
    *out = spectrum.bands(sample_rate);
}

fn valid_pcm(bytes: &[u8], channels: usize) -> bool {
    channels > 0 && channels <= 8 && bytes.len() <= 256 * 1024 && bytes.len() >= channels * 2
}

fn sample(frame: &[u8], channel: usize) -> f32 {
    let index = channel * 2;
    i16::from_le_bytes([frame[index], frame[index + 1]]) as f32 / 32768.0
}

/// Log-spaced band edges in Hz. All zeros when the rate cannot resolve the range.
fn band_edges(sample_rate: u32) -> [f32; BANDS + 1] {
    let mut edges = [0.0; BANDS + 1];
    let top = MAX_HZ.min(sample_rate as f32 / 2.0 * 0.9);
    if sample_rate < 8_000 || top <= MIN_HZ {
        return edges;
    }
    let ratio = top / MIN_HZ;
    for (index, edge) in edges.iter_mut().enumerate() {
        *edge = MIN_HZ * ratio.powf(index as f32 / BANDS as f32);
    }
    edges
}

struct Spectrum {
    ring: [f32; FFT_SIZE],
    /// Index of the oldest sample, which is also the next write position.
    cursor: usize,
    window: [f32; FFT_SIZE],
    cos: [f32; HALF],
    sin: [f32; HALF],
    /// Scales |X_k|² so a full-scale sine's peak bin reads its amplitude squared.
    power_scale: f32,
}

impl Spectrum {
    fn new() -> Self {
        let mut window = [0.0_f32; FFT_SIZE];
        let mut window_sum = 0.0_f64;
        for (index, weight) in window.iter_mut().enumerate() {
            let phase = std::f64::consts::TAU * index as f64 / FFT_SIZE as f64;
            *weight = (0.5 - 0.5 * phase.cos()) as f32;
            window_sum += *weight as f64;
        }
        let mut cos = [0.0_f32; HALF];
        let mut sin = [0.0_f32; HALF];
        for (index, (c, s)) in cos.iter_mut().zip(sin.iter_mut()).enumerate() {
            let phase = std::f64::consts::TAU * index as f64 / FFT_SIZE as f64;
            *c = phase.cos() as f32;
            *s = phase.sin() as f32;
        }
        Self {
            ring: [0.0; FFT_SIZE],
            cursor: 0,
            window,
            cos,
            sin,
            power_scale: (4.0 / (window_sum * window_sum)) as f32,
        }
    }

    /// Appends the newest frames as mono. Only the last window's worth is ever kept.
    fn push(&mut self, bytes: &[u8], channels: usize) {
        if !valid_pcm(bytes, channels) {
            return;
        }
        let frames = bytes.len() / (channels * 2);
        for frame in bytes
            .chunks_exact(channels * 2)
            .skip(frames.saturating_sub(FFT_SIZE))
        {
            let mono = (0..channels)
                .map(|channel| sample(frame, channel))
                .sum::<f32>()
                / channels as f32;
            self.ring[self.cursor] = mono;
            self.cursor = (self.cursor + 1) % FFT_SIZE;
        }
    }

    fn bands(&self, sample_rate: u32) -> [i32; BANDS] {
        let mut out = [0_i32; BANDS];
        let edges = band_edges(sample_rate);
        if edges[BANDS] == 0.0 {
            return out;
        }
        let mut re = [0.0_f32; FFT_SIZE];
        let mut im = [0.0_f32; FFT_SIZE];
        for (index, slot) in re.iter_mut().enumerate() {
            *slot = self.ring[(self.cursor + index) % FFT_SIZE] * self.window[index];
        }
        fft(&mut re, &mut im, &self.cos, &self.sin);
        let mut power = [0.0_f32; HALF];
        for ((p, &r), &i) in power.iter_mut().zip(re.iter()).zip(im.iter()) {
            *p = (r * r + i * i) * self.power_scale;
        }
        let bin_hz = sample_rate as f32 / FFT_SIZE as f32;
        for (band, slot) in out.iter_mut().enumerate() {
            let (low, high) = (edges[band], edges[band + 1]);
            let first = ((low / bin_hz).round() as usize).clamp(1, HALF - 1);
            let last = ((high / bin_hz).floor() as usize).clamp(first, HALF - 1);
            let bins = &power[first..=last];
            let mean = bins.iter().sum::<f32>() / bins.len() as f32;
            let center = (low * high).sqrt();
            let tilt = (TILT_DB_PER_OCTAVE * (center / 250.0).log2())
                .clamp(TILT_RANGE_DB.0, TILT_RANGE_DB.1);
            let db = 10.0 * mean.max(1e-12).log10() + GAIN_DB + tilt;
            let unit = ((db + DISPLAY_RANGE_DB) / DISPLAY_RANGE_DB).clamp(0.0, 1.0);
            *slot = (unit * 65535.0).round() as i32;
        }
        out
    }
}

/// In-place iterative radix-2 forward FFT.
fn fft(re: &mut [f32; FFT_SIZE], im: &mut [f32; FFT_SIZE], cos: &[f32; HALF], sin: &[f32; HALF]) {
    let mut j = 0;
    for i in 1..FFT_SIZE {
        let mut bit = FFT_SIZE >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= FFT_SIZE {
        let half = len / 2;
        let step = FFT_SIZE / len;
        for start in (0..FFT_SIZE).step_by(len) {
            for k in 0..half {
                let (wr, wi) = (cos[k * step], -sin[k * step]);
                let (a, b) = (start + k, start + k + half);
                let tr = re[b] * wr - im[b] * wi;
                let ti = re[b] * wi + im[b] * wr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    #[test]
    fn silence_stereo_mono_and_malformed_frames_are_bounded() {
        assert_eq!(pcm16(&[0; 32], 2), 0);
        assert_eq!(pcm16(&[1, 2], 0), 0);
        assert_eq!(pcm16(&[1, 2], 9), 0);
        let mut bytes = vec![];
        for _ in 0..128 {
            bytes.extend(16384_i16.to_le_bytes());
            bytes.extend(1638_i16.to_le_bytes());
        }
        let v = pcm16(&bytes, 2);
        assert!((v & 65535) > ((v >> 16) & 65535) + 15000);
        let m = pcm16(&[0, 64, 0, 64], 1);
        assert_eq!(m & 65535, (m >> 16) & 65535);
    }

    /// Interleaved stereo PCM16 for a sine at `amplitude` (0..1 full scale).
    fn sine_stereo(freq: f32, rate: u32, frames: usize, amplitude: f32) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(frames * 4);
        for n in 0..frames {
            let value = amplitude * (TAU * freq * n as f32 / rate as f32).sin();
            let pcm = (value * 32767.0) as i16;
            bytes.extend(pcm.to_le_bytes());
            bytes.extend(pcm.to_le_bytes());
        }
        bytes
    }

    fn argmax(values: &[i32]) -> usize {
        values
            .iter()
            .enumerate()
            .max_by_key(|(_, value)| **value)
            .map(|(index, _)| index)
            .unwrap()
    }

    #[test]
    fn silence_and_unresolvable_rates_produce_zero_bands() {
        let mut spectrum = Spectrum::new();
        spectrum.push(&[0; 8192], 2);
        assert_eq!(spectrum.bands(44_100), [0; BANDS]);
        let mut loud = Spectrum::new();
        loud.push(&sine_stereo(440.0, 44_100, 2048, 0.9), 2);
        assert_eq!(loud.bands(4_000), [0; BANDS]);
        assert_eq!(loud.bands(0), [0; BANDS]);
    }

    #[test]
    fn a_tone_lands_in_the_band_that_contains_it() {
        let mut spectrum = Spectrum::new();
        spectrum.push(&sine_stereo(1_000.0, 44_100, 2048, 0.5), 2);
        let bands = spectrum.bands(44_100);
        let edges = band_edges(44_100);
        let expected = (0..BANDS)
            .find(|&band| edges[band] <= 1_000.0 && 1_000.0 < edges[band + 1])
            .unwrap();
        assert_eq!(argmax(&bands), expected);
        assert!(bands[expected] > 40_000, "tone too weak: {bands:?}");
        assert!(bands[4] < 5_000, "tone leaked into the bass: {bands:?}");
    }

    #[test]
    fn bass_and_treble_tones_move_to_the_matching_ends() {
        let mut low = Spectrum::new();
        low.push(&sine_stereo(90.0, 48_000, 2048, 0.8), 2);
        let mut high = Spectrum::new();
        high.push(&sine_stereo(9_000.0, 48_000, 2048, 0.8), 2);
        let low_bands = low.bands(48_000);
        let high_bands = high.bands(48_000);
        assert!(argmax(&low_bands) < 8, "{low_bands:?}");
        assert!(argmax(&high_bands) > 15, "{high_bands:?}");
    }

    #[test]
    fn short_and_malformed_buffers_are_ignored_without_panicking() {
        let mut spectrum = Spectrum::new();
        spectrum.push(&[1, 2], 0);
        spectrum.push(&[1, 2, 3], 9);
        spectrum.push(&[1, 2, 3], 2);
        spectrum.push(&vec![0; 300 * 1024], 2);
        assert_eq!(spectrum.bands(44_100), [0; BANDS]);
        let mut out = [7; BANDS];
        spectrum_pcm16(&[0; 16], 2, 44_100, &mut out);
        assert!(out.iter().all(|value| (0..=65535).contains(value)));
    }

    #[test]
    fn only_the_newest_window_contributes_after_a_long_buffer() {
        let mut spectrum = Spectrum::new();
        spectrum.push(&sine_stereo(1_000.0, 44_100, 4096, 0.5), 2);
        spectrum.push(&[0; 4096 * 4], 2);
        assert_eq!(spectrum.bands(44_100), [0; BANDS]);
    }
}
