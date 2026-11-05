//! Skippable first-run setup (M9 parity with the Android onboarding flow): welcome,
//! look & feel, playback, account and a summary. Changes are drafted here and saved
//! through the normal settings path when the user finishes or skips; every choice
//! stays in Settings. Completion is recorded per [`VERSION`] in the state snapshot.
use vidcue_core::{Config, equalizer};
use vidcue_services::theme::{Palette, THEMES};

/// First-run setup is shown once per version; bump it to show the flow again.
pub const VERSION: u32 = 1;

pub const WELCOME: usize = 0;
pub const LOOK: usize = 1;
pub const PLAYBACK: usize = 2;
pub const ACCOUNT: usize = 3;
pub const READY: usize = 4;
pub const TITLES: [&str; 5] = [
    "Welcome",
    "Look & feel",
    "Playback",
    "Your account",
    "You're all set",
];
const SPONSORBLOCK: [&str; 3] = ["skip", "mark", "off"];
/// Playback-step row of the equalizer preset; band rows follow it for custom presets.
const EQUALIZER_ROW: usize = 4;
const BAND_LABELS: [&str; 10] = [
    "31 Hz", "62 Hz", "125 Hz", "250 Hz", "500 Hz", "1 kHz", "2 kHz", "4 kHz", "8 kHz", "16 kHz",
];

pub struct Setup {
    pub step: usize,
    /// Selected row on steps that have rows.
    pub row: usize,
    pub draft: Config,
    /// Palette captured at the first theme preview, restored if the theme ends up
    /// unchanged.
    pub original_palette: Option<Palette>,
}

/// One adjustable line of a step: label, current value and an optional hint.
pub struct Row {
    pub label: &'static str,
    pub value: String,
    pub hint: &'static str,
}

fn on_off(value: bool) -> String {
    if value { "On" } else { "Off" }.into()
}

fn decibels(gain: f64) -> String {
    format!("{gain:+} dB")
}

impl Setup {
    pub fn new(config: &Config) -> Self {
        Self {
            step: WELCOME,
            row: 0,
            draft: config.clone(),
            original_palette: None,
        }
    }
    pub fn is_last(&self) -> bool {
        self.step == READY
    }
    pub fn next(&mut self) {
        self.step = (self.step + 1).min(READY);
        self.row = 0;
    }
    /// Returns false on the first step, where Back has nowhere to go.
    pub fn back(&mut self) -> bool {
        if self.step == WELCOME {
            return false;
        }
        self.step -= 1;
        self.row = 0;
        true
    }
    pub fn progress(&self) -> String {
        format!("Step {} of {}", self.step + 1, TITLES.len())
    }
    pub fn rows(&self, connected: bool) -> Vec<Row> {
        let c = &self.draft;
        match self.step {
            LOOK => vec![
                Row {
                    label: "Theme",
                    value: c.theme.clone(),
                    hint: "Applies right away",
                },
                Row {
                    label: "Reduce motion",
                    value: on_off(c.reduced_motion),
                    hint: "Fewer animations and still visualizers",
                },
                Row {
                    label: "Thumbnails",
                    value: on_off(c.thumbnails),
                    hint: "Artwork in lists and the player",
                },
            ],
            PLAYBACK => vec![
                Row {
                    label: "Autoplay related videos",
                    value: on_off(c.autoplay_related),
                    hint: "Keep going when the queue ends",
                },
                Row {
                    label: "Resume where you left off",
                    value: on_off(c.resume_on_play),
                    hint: "Partly watched videos continue",
                },
                Row {
                    label: "Save local playback positions",
                    value: on_off(c.playback_history),
                    hint: "Needed for resume; stays on this computer",
                },
                Row {
                    label: "SponsorBlock",
                    value: c.playback_sponsorblock.clone(),
                    hint: "skip, mark or off",
                },
                Row {
                    label: "Equalizer",
                    value: c.equalizer.clone(),
                    hint: "Presets, or \"custom\" to shape your own curve below",
                },
            ]
            .into_iter()
            .chain(
                c.equalizer_custom
                    .get(&c.equalizer)
                    .into_iter()
                    .flatten()
                    .zip(BAND_LABELS)
                    .map(|(gain, label)| Row {
                        label,
                        value: decibels(*gain),
                        hint: "",
                    }),
            )
            .collect(),
            ACCOUNT => vec![Row {
                label: if connected {
                    "Reconnect browser or cookie file"
                } else {
                    "Connect browser or cookie file"
                },
                value: if connected {
                    "Signed in"
                } else {
                    "Not signed in"
                }
                .into(),
                hint: "Press Space to finish setup and connect",
            }],
            _ => Vec::new(),
        }
    }
    pub fn move_row(&mut self, delta: isize, connected: bool) {
        let count = self.rows(connected).len();
        if count > 0 {
            self.row = (self.row as isize + delta).clamp(0, count as isize - 1) as usize;
        }
    }
    /// Changes the selected row by `delta` (cycling choices). Returns true when the
    /// theme changed so the caller can preview it.
    pub fn change(&mut self, delta: isize) -> bool {
        let c = &mut self.draft;
        let cycle = |values: &[&str], current: &str| {
            let index = values.iter().position(|v| *v == current).unwrap_or(0) as isize;
            values[(index + delta).rem_euclid(values.len() as isize) as usize].to_string()
        };
        match (self.step, self.row) {
            (LOOK, 0) => {
                c.theme = cycle(THEMES, &c.theme);
                return true;
            }
            (LOOK, 1) => c.reduced_motion = !c.reduced_motion,
            (LOOK, 2) => c.thumbnails = !c.thumbnails,
            (PLAYBACK, 0) => c.autoplay_related = !c.autoplay_related,
            (PLAYBACK, 1) => c.resume_on_play = !c.resume_on_play,
            (PLAYBACK, 2) => c.playback_history = !c.playback_history,
            (PLAYBACK, 3) => {
                c.playback_sponsorblock = cycle(&SPONSORBLOCK, &c.playback_sponsorblock)
            }
            (PLAYBACK, EQUALIZER_ROW) => {
                // Offer a fresh "custom" curve (flat) alongside saved custom presets,
                // and drop it again when the user moves on without shaping it.
                let mut names = equalizer::names(&c.equalizer_custom);
                if !c.equalizer_custom.contains_key(equalizer::CUSTOM) {
                    names.push(equalizer::CUSTOM.into());
                }
                let names = names.iter().map(String::as_str).collect::<Vec<_>>();
                let next = cycle(&names, &c.equalizer);
                if c.equalizer == equalizer::CUSTOM
                    && c.equalizer_custom
                        .get(equalizer::CUSTOM)
                        .is_some_and(|g| g.iter().all(|g| *g == 0.0))
                {
                    c.equalizer_custom.remove(equalizer::CUSTOM);
                }
                if next == equalizer::CUSTOM {
                    c.equalizer_custom
                        .entry(next.clone())
                        .or_insert_with(|| vec![0.0; equalizer::BANDS.len()]);
                }
                c.equalizer = next;
            }
            (PLAYBACK, row) if row > EQUALIZER_ROW => {
                if let Some(gain) = c
                    .equalizer_custom
                    .get_mut(&c.equalizer)
                    .and_then(|gains| gains.get_mut(row - EQUALIZER_ROW - 1))
                {
                    *gain = (gain.round() + delta as f64)
                        .clamp(-equalizer::MAX_GAIN, equalizer::MAX_GAIN);
                }
            }
            _ => {}
        }
        false
    }
    /// The Ready step's summary of what was chosen.
    pub fn summary(&self, connected: bool) -> Vec<(&'static str, String)> {
        let c = &self.draft;
        vec![
            ("Theme", c.theme.clone()),
            ("Reduce motion", on_off(c.reduced_motion)),
            ("Autoplay related", on_off(c.autoplay_related)),
            ("Resume videos", on_off(c.resume_on_play)),
            ("SponsorBlock", c.playback_sponsorblock.clone()),
            ("Equalizer", c.equalizer.clone()),
            (
                "Account",
                if connected {
                    "Signed in"
                } else {
                    "Not signed in"
                }
                .into(),
            ),
        ]
    }
    /// True when the draft differs from `config` and needs saving.
    pub fn changed(&self, config: &Config) -> bool {
        toml::to_string(&self.draft).ok() != toml::to_string(config).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> Setup {
        Setup::new(&Config::default())
    }
    #[test]
    fn steps_move_forward_and_back_within_bounds() {
        let mut s = setup();
        assert!(!s.back(), "Back on the first step has nowhere to go");
        for expected in 1..TITLES.len() {
            s.next();
            assert_eq!(s.step, expected);
        }
        assert!(s.is_last());
        s.next();
        assert_eq!(s.step, READY);
        assert_eq!(s.progress(), "Step 5 of 5");
        assert!(s.back());
        assert_eq!(s.step, ACCOUNT);
    }
    #[test]
    fn rows_change_the_draft_only_and_theme_changes_are_reported() {
        let config = Config::default();
        let mut s = setup();
        s.next();
        assert_eq!(s.rows(false).len(), 3);
        assert!(s.change(1), "theme row previews");
        assert_ne!(s.draft.theme, config.theme);
        assert!(s.change(-1));
        assert_eq!(s.draft.theme, config.theme);
        assert!(!s.changed(&config));
        s.move_row(1, false);
        assert!(!s.change(1));
        assert!(s.draft.reduced_motion);
        s.move_row(10, false);
        assert_eq!(s.row, 2);
        s.next();
        s.change(1);
        assert!(!s.draft.autoplay_related);
        s.move_row(3, false);
        s.change(1);
        assert_eq!(s.draft.playback_sponsorblock, "mark");
        s.change(-1);
        s.change(-1);
        assert_eq!(s.draft.playback_sponsorblock, "off");
        assert!(s.changed(&config));
        assert!(config.autoplay_related && !config.reduced_motion);
        s.draft.validate().unwrap();
    }
    #[test]
    fn equalizer_presets_and_a_custom_curve_are_chosen_in_the_playback_step() {
        let config = Config::default();
        let mut s = setup();
        s.step = PLAYBACK;
        assert_eq!(s.rows(false).len(), 5);
        s.move_row(EQUALIZER_ROW as isize, false);
        assert_eq!(s.rows(false)[EQUALIZER_ROW].value, "off");
        s.change(1);
        assert_eq!(s.draft.equalizer, "flat");
        s.change(1);
        assert_eq!(s.draft.equalizer, "bass-boost");
        assert_eq!(s.rows(false).len(), 5, "built-in presets have no band rows");
        assert!(s.changed(&config));
        s.draft.validate().unwrap();
        // Back from "off" wraps to the new custom curve, which shows ten band rows.
        s.change(-1);
        s.change(-1);
        s.change(-1);
        assert_eq!(s.draft.equalizer, "custom");
        let rows = s.rows(false);
        assert_eq!(rows.len(), 15);
        assert_eq!((rows[5].label, rows[5].value.as_str()), ("31 Hz", "+0 dB"));
        s.move_row(1, false);
        for _ in 0..20 {
            s.change(1);
        }
        assert_eq!(s.rows(false)[5].value, "+12 dB", "gains are clamped");
        s.move_row(20, false);
        assert_eq!(s.row, 14);
        s.change(-1);
        assert_eq!(s.draft.equalizer_custom["custom"][9], -1.0);
        s.draft.validate().unwrap();
        assert!(
            s.summary(false)
                .iter()
                .any(|(k, v)| *k == "Equalizer" && v == "custom")
        );
        // A shaped curve stays; an untouched one is dropped when moving on.
        s.row = EQUALIZER_ROW;
        s.change(1);
        assert_eq!(s.draft.equalizer, "off");
        assert!(s.draft.equalizer_custom.contains_key("custom"));
        let mut fresh = setup();
        fresh.step = PLAYBACK;
        fresh.row = EQUALIZER_ROW;
        fresh.change(-1);
        assert_eq!(fresh.draft.equalizer, "custom");
        fresh.change(1);
        assert!(!fresh.changed(&config));
    }
    #[test]
    fn account_and_summary_reflect_the_session() {
        let mut s = setup();
        s.step = ACCOUNT;
        assert_eq!(s.rows(false)[0].value, "Not signed in");
        assert_eq!(s.rows(true)[0].value, "Signed in");
        s.step = READY;
        assert!(s.rows(true).is_empty());
        assert!(
            s.summary(true)
                .iter()
                .any(|(k, v)| *k == "Account" && v == "Signed in")
        );
    }
}
