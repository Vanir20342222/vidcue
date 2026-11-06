//! Android playback equalizer: the engine's saved settings select the curve and the
//! Media3 audio processor feeds 16-bit PCM through [`process_pcm16`]. The filter
//! maths are shared with desktop (`vidcue_core::equalizer`).
use std::sync::Mutex;
use vidcue_core::{Config, equalizer};

/// The selected curve plus a filter bank built for the current PCM format.
#[derive(Default)]
pub struct Processor {
    gains: Option<[f64; 10]>,
    format: (u32, usize),
    bank: Option<equalizer::Bank>,
}
impl Processor {
    pub const fn new() -> Self {
        Self {
            gains: None,
            format: (0, 0),
            bank: None,
        }
    }
    /// Applies the saved preset; returns whether any filtering is active.
    pub fn configure(&mut self, config: &Config) -> bool {
        let gains = equalizer::gains(&config.equalizer, &config.equalizer_custom)
            .filter(|g| g.iter().any(|v| *v != 0.0));
        if self.gains != gains {
            self.gains = gains;
            self.bank = None;
            self.format = (0, 0);
        }
        gains.is_some()
    }
    pub fn enabled(&self) -> bool {
        self.gains.is_some()
    }
    /// Clears filter memory after a seek or a new stream.
    pub fn reset(&mut self) {
        if let Some(bank) = self.bank.as_mut() {
            bank.reset();
        }
    }
    /// Filters interleaved 16-bit PCM in place. Off leaves the samples untouched.
    pub fn process_pcm16(&mut self, bytes: &mut [u8], channels: usize, sample_rate: u32) {
        let Some(gains) = self.gains else { return };
        if self.format != (sample_rate, channels) {
            self.format = (sample_rate, channels);
            self.bank = equalizer::Bank::new(&gains, sample_rate, channels);
        }
        if let Some(bank) = self.bank.as_mut() {
            bank.process_pcm16(bytes);
        }
    }
}

/// The app's one playback processor, shared by the engine and Media3's audio thread.
static PLAYBACK: Mutex<Processor> = Mutex::new(Processor::new());
fn playback() -> std::sync::MutexGuard<'static, Processor> {
    PLAYBACK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
pub fn configure(config: &Config) -> bool {
    playback().configure(config)
}
pub fn enabled() -> bool {
    playback().enabled()
}
pub fn reset() {
    playback().reset();
}
pub fn process_pcm16(bytes: &mut [u8], channels: usize, sample_rate: u32) {
    playback().process_pcm16(bytes, channels, sample_rate);
}

/// Bands, limits and built-in presets for the settings and onboarding screens.
pub fn catalog() -> serde_json::Value {
    serde_json::json!({
        "bands": equalizer::BANDS,
        "max_gain": equalizer::MAX_GAIN,
        "max_custom": equalizer::MAX_CUSTOM,
        "presets": equalizer::PRESETS
            .iter()
            .map(|(name, gains)| serde_json::json!({"name": name, "gains": gains}))
            .collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_settings_switch_processing_on_and_off_and_catalog_lists_presets() {
        let mut p = Processor::new();
        let mut config = Config::default();
        assert!(!p.configure(&config));
        let mut silent = vec![0u8; 8];
        let mut loud = 1000i16.to_le_bytes().repeat(4);
        let original = loud.clone();
        p.process_pcm16(&mut loud, 2, 48_000);
        assert_eq!(loud, original, "off passes audio through untouched");
        config.equalizer = "flat".into();
        assert!(!p.configure(&config), "flat needs no processing");
        config.equalizer = "bass-boost".into();
        assert!(p.configure(&config) && p.enabled());
        p.process_pcm16(&mut loud, 2, 48_000);
        assert_ne!(loud, original);
        // Filter memory rings on into the next buffer until a seek resets it.
        p.reset();
        p.process_pcm16(&mut silent, 2, 48_000);
        assert_eq!(silent, vec![0u8; 8]);
        config.equalizer_custom.insert(
            "night".into(),
            vec![-3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        );
        config.equalizer = "night".into();
        assert!(p.configure(&config));
        config.equalizer = "off".into();
        assert!(!p.configure(&config) && !p.enabled());
        let catalog = catalog();
        assert_eq!(catalog["bands"].as_array().unwrap().len(), 10);
        assert_eq!(catalog["presets"][0]["name"], "flat");
        assert_eq!(catalog["presets"][1]["gains"][0], 6.0);
    }
}
