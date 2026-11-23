//! Jam clock and drift rule (PROTOCOL.md §5.5).
use crate::protocol::JamPlayback;

/// Pong samples with a round trip above this are discarded.
pub const MAX_RTT_MS: i64 = 1000;
/// Samples taken when joining a room, 200 ms apart.
pub const JOIN_SAMPLES: usize = 5;
/// Samples taken every 60 s and after a reconnect.
pub const RESAMPLES: usize = 3;
pub const SAMPLE_GAP_MS: u64 = 200;
pub const RESAMPLE_EVERY_MS: u64 = 60_000;
/// Drift tolerated before seeking.
pub const MAX_DRIFT_MS: i64 = 400;
/// Seek this far ahead of the expected position to cover seek latency.
pub const SEEK_LEAD_MS: u64 = 150;
/// Two seeks are never issued within this window, so slow buffering cannot loop.
pub const SEEK_BACKOFF_MS: i64 = 3000;

/// Estimates `server time - local time` from ping/pong samples.
#[derive(Clone, Debug, Default)]
pub struct OffsetEstimator {
    /// `(rtt, offset)` of the current sampling round.
    round: Vec<(i64, i64)>,
    offset: Option<i64>,
    rtt: Option<i64>,
}

impl OffsetEstimator {
    /// Starts a new sampling round; the last estimate stays in use until it ends.
    pub fn begin_round(&mut self) {
        self.round.clear();
    }
    /// Records a pong for a ping sent at local `t0`, received at local `t1`.
    /// Returns false when the sample was discarded.
    pub fn sample(&mut self, t0: i64, t1: i64, server_ms: i64) -> bool {
        let rtt = t1 - t0;
        if !(0..=MAX_RTT_MS).contains(&rtt) {
            return false;
        }
        let offset = server_ms - (t0 + rtt / 2);
        self.round.push((rtt, offset));
        // Keep the offset of the sample with the smallest round trip in this round.
        if let Some(&(rtt, offset)) = self.round.iter().min_by_key(|(rtt, _)| *rtt) {
            self.offset = Some(offset);
            self.rtt = Some(rtt);
        }
        true
    }
    /// Uses the `hello`/`pong` server time without a round trip, when nothing better exists.
    pub fn seed(&mut self, local_ms: i64, server_ms: i64) {
        if self.offset.is_none() {
            self.offset = Some(server_ms - local_ms);
        }
    }
    pub fn offset(&self) -> Option<i64> {
        self.offset
    }
    pub fn rtt(&self) -> Option<i64> {
        self.rtt
    }
    /// Server time now, given local time.
    pub fn server_now(&self, local_ms: i64) -> i64 {
        local_ms + self.offset.unwrap_or(0)
    }
}

/// Expected playback position at server time `server_now` (§5.3).
pub fn expected_position(playback: &JamPlayback, server_now: i64) -> u64 {
    let base = playback.position_ms as f64;
    let position = if playback.state == "playing" {
        base + (server_now - playback.at_server_ms) as f64 * playback.rate
    } else {
        base
    };
    let mut position = position.max(0.0);
    if let Some(item) = &playback.item
        && let Some(duration) = item.media.duration
    {
        position = position.min(duration as f64 * 1000.0);
    }
    position as u64
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DriftAction {
    None,
    /// Start the room's current media at the target position.
    Load,
    Play,
    Pause,
    /// Seek to this position (target plus [`SEEK_LEAD_MS`]).
    Seek(u64),
}

impl DriftAction {
    pub fn name(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Load => "load",
            Self::Play => "play",
            Self::Pause => "pause",
            Self::Seek(_) => "seek",
        }
    }
}

/// What the local player reports.
#[derive(Clone, Debug, Default)]
pub struct PlayerState {
    pub media_key: Option<String>,
    pub position_ms: Option<u64>,
    pub playing: Option<bool>,
}

/// Applies the drift rule; `last_seek` is the local time of the last issued seek and is
/// updated when this returns [`DriftAction::Seek`].
pub fn drift_action(
    playback: &JamPlayback,
    player: &PlayerState,
    server_now: i64,
    local_now: i64,
    last_seek: &mut Option<i64>,
) -> DriftAction {
    let Some(item) = &playback.item else {
        // Nothing current in the room: a playing local player pauses.
        return if player.playing == Some(true) {
            DriftAction::Pause
        } else {
            DriftAction::None
        };
    };
    if player.media_key.as_deref() != Some(item.media_key.as_str()) {
        return DriftAction::Load;
    }
    let playing = playback.state == "playing";
    if let Some(local) = player.playing
        && local != playing
    {
        return if playing {
            DriftAction::Play
        } else {
            DriftAction::Pause
        };
    }
    let expected = expected_position(playback, server_now);
    let Some(position) = player.position_ms else {
        return DriftAction::None;
    };
    let drift = position as i64 - expected as i64;
    if drift.abs() <= MAX_DRIFT_MS {
        return DriftAction::None;
    }
    if last_seek.is_some_and(|at| local_now - at < SEEK_BACKOFF_MS) {
        return DriftAction::None;
    }
    *last_seek = Some(local_now);
    DriftAction::Seek(expected + if playing { SEEK_LEAD_MS } else { 0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{JamItem, JamPlayback};

    fn playback(state: &str, position_ms: u64, at: i64) -> JamPlayback {
        JamPlayback {
            item: Some(JamItem {
                item_id: uuid::Uuid::nil(),
                media: vidcue_core::Media {
                    url: "https://www.youtube.com/watch?v=abcdefghijk".into(),
                    duration: Some(300),
                    ..Default::default()
                },
                media_key: "youtube:abcdefghijk".into(),
                added_by: uuid::Uuid::nil(),
            }),
            state: state.into(),
            position_ms,
            at_server_ms: at,
            rate: 1.0,
            version: 1,
        }
    }

    #[test]
    fn offset_uses_the_smallest_rtt_and_discards_slow_samples() {
        let mut clock = OffsetEstimator::default();
        assert!(clock.sample(1000, 1100, 5050)); // rtt 100, offset 4000
        assert_eq!(clock.offset(), Some(4000));
        assert!(clock.sample(2000, 2020, 6013)); // rtt 20, offset 4003
        assert_eq!(clock.offset(), Some(4003));
        assert!(clock.sample(3000, 3300, 7000)); // rtt 300: kept, not best
        assert_eq!(clock.offset(), Some(4003));
        assert!(!clock.sample(4000, 5500, 9000)); // rtt 1500 discarded
        assert!(!clock.sample(4000, 3999, 9000)); // negative
        assert_eq!(clock.rtt(), Some(20));
        assert_eq!(clock.server_now(10_000), 14_003);
        clock.begin_round();
        assert!(clock.sample(5000, 5200, 9500)); // a new round replaces the estimate
        assert_eq!(clock.offset(), Some(4400));
        let mut seeded = OffsetEstimator::default();
        seeded.seed(100, 600);
        assert_eq!(seeded.offset(), Some(500));
        seeded.seed(100, 900);
        assert_eq!(seeded.offset(), Some(500));
    }

    #[test]
    fn expected_position_advances_only_while_playing() {
        let p = playback("playing", 51_000, 1_000_000);
        assert_eq!(expected_position(&p, 1_002_000), 53_000);
        let mut fast = p.clone();
        fast.rate = 2.0;
        assert_eq!(expected_position(&fast, 1_002_000), 55_000);
        assert_eq!(
            expected_position(&playback("paused", 51_000, 0), 99_999),
            51_000
        );
        // Clamped to the media duration.
        assert_eq!(expected_position(&p, 10_000_000), 300_000);
    }

    #[test]
    fn drift_rule_decisions() {
        let p = playback("playing", 10_000, 0);
        let key = Some("youtube:abcdefghijk".to_owned());
        let mut last = None;
        let player = |key: Option<String>, pos: u64, playing: bool| PlayerState {
            media_key: key,
            position_ms: Some(pos),
            playing: Some(playing),
        };
        assert_eq!(
            drift_action(&p, &player(None, 0, false), 0, 0, &mut last),
            DriftAction::Load
        );
        assert_eq!(
            drift_action(&p, &player(key.clone(), 10_000, false), 0, 0, &mut last),
            DriftAction::Play
        );
        let paused = playback("paused", 10_000, 0);
        assert_eq!(
            drift_action(&paused, &player(key.clone(), 10_000, true), 0, 0, &mut last),
            DriftAction::Pause
        );
        // Within 400 ms: nothing.
        assert_eq!(
            drift_action(&p, &player(key.clone(), 10_390, true), 0, 0, &mut last),
            DriftAction::None
        );
        // Over: seek to expected + lead, then back off for 3 s.
        assert_eq!(
            drift_action(&p, &player(key.clone(), 11_000, true), 0, 0, &mut last),
            DriftAction::Seek(10_150)
        );
        assert_eq!(
            drift_action(&p, &player(key.clone(), 11_000, true), 0, 2_999, &mut last),
            DriftAction::None
        );
        assert_eq!(
            drift_action(&p, &player(key.clone(), 9_000, true), 0, 3_000, &mut last),
            DriftAction::Seek(10_150)
        );
        // Paused rooms seek without the lead.
        let mut last = None;
        assert_eq!(
            drift_action(&paused, &player(key.clone(), 0, false), 0, 0, &mut last),
            DriftAction::Seek(10_000)
        );
        // Nothing current: pause a playing player.
        let mut stopped = p.clone();
        stopped.item = None;
        stopped.state = "stopped".into();
        assert_eq!(
            drift_action(&stopped, &player(key, 0, true), 0, 0, &mut last),
            DriftAction::Pause
        );
    }
}
