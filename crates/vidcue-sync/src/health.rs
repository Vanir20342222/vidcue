//! Playback health reports (PROTOCOL.md §6.2, §7.7): local classification of error text
//! into a fixed category and a bounded report buffer. Error text never leaves this module.
use crate::protocol::{HealthCategory, HealthKind, HealthReport};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Reports kept while offline; the oldest are dropped first.
pub const MAX_BUFFERED: usize = 500;
/// Reports per upload (`POST /api/health/reports` accepts at most 100).
pub const BATCH: usize = 100;
/// Durations above this are not reported (the server accepts 0..=600000).
pub const MAX_DURATION_MS: u64 = 600_000;

use HealthCategory as C;

/// First matching row wins; every needle is lowercase.
const TABLE: &[(&[&str], HealthCategory)] = &[
    (
        &["cancelled", "canceled", "request cancelled"],
        C::Cancelled,
    ),
    (
        &[
            "no space left",
            "enospc",
            "disk full",
            "storage full",
            "not enough storage",
        ],
        C::StorageFull,
    ),
    (
        &[
            "no such file",
            "file not found",
            "filenotfound",
            "enoent",
            "offline file is missing",
        ],
        C::OfflineFileMissing,
    ),
    (
        &[
            "confirm your age",
            "age-restricted",
            "age restricted",
            "inappropriate for some users",
        ],
        C::AgeRestricted,
    ),
    (
        &["members-only", "members only", "join this channel"],
        C::MembersOnly,
    ),
    (
        &[
            "available in your country",
            "blocked it in your country",
            "geo-restricted",
            "geo restricted",
            "geoblock",
        ],
        C::GeoBlocked,
    ),
    (&["drm", "widevine", "protected content"], C::Drm),
    (
        &[
            "n challenge",
            "nsig",
            "js challenge",
            "javascript challenge",
            "jsc ",
            "js runtime",
            "javascript solver",
        ],
        C::JsChallengeFailed,
    ),
    (&["signature"], C::SignatureFailed),
    (
        &[
            "requested format is not available",
            "requested format not available",
            "no video formats",
            "no formats",
            "format unavailable",
        ],
        C::FormatUnavailable,
    ),
    (
        &["live stream", "livestream", "is live", "live event"],
        C::LiveUnsupported,
    ),
    (
        &["429", "too many requests", "rate limit", "rate-limit"],
        C::Http429,
    ),
    (&["403", "forbidden"], C::Http403),
    (&["404", "not found"], C::Http404),
    (
        &["timed out", "timeout", "time out", "deadline exceeded"],
        C::NetworkTimeout,
    ),
    (&["certificate", "tls", "ssl", "handshake"], C::Tls),
    (
        &[
            "unreachable",
            "connection refused",
            "connection reset",
            "dns",
            "failed to lookup",
            "unknown host",
            "unable to resolve host",
            "no route to host",
            "network is down",
            "error sending request",
            "unknownhostexception",
        ],
        C::NetworkUnreachable,
    ),
    (
        &["decoder", "codec", "mediacodec", "renderer"],
        C::DecoderError,
    ),
    (
        &["audiotrack", "audio output", "audio sink", "audiosink"],
        C::AudioOutputError,
    ),
    (
        &[
            "unable to extract",
            "extractor",
            "player response",
            "playability",
            "video unavailable",
        ],
        C::ExtractorFailed,
    ),
];

/// Maps error text to a category. Pure and table-driven; the text is not kept.
pub fn classify(error_text: &str) -> HealthCategory {
    let lower = error_text.to_ascii_lowercase();
    // HTTP 5xx: a three-digit 5xx code next to "http" or "status".
    if (lower.contains("http") || lower.contains("status")) && has_5xx(&lower) {
        // Specific causes above still win (for example "403" in a 503 retry message is
        // unlikely, but a "429" is more useful than a generic 5xx).
        if let Some(category) = table(&lower).filter(|c| matches!(c, C::Http429 | C::Cancelled)) {
            return category;
        }
        return C::Http5xx;
    }
    table(&lower).unwrap_or(C::Unknown)
}

fn table(lower: &str) -> Option<HealthCategory> {
    TABLE
        .iter()
        .find(|(needles, _)| needles.iter().any(|n| lower.contains(n)))
        .map(|(_, category)| *category)
}

fn has_5xx(lower: &str) -> bool {
    let bytes = lower.as_bytes();
    bytes.windows(3).enumerate().any(|(i, w)| {
        w[0] == b'5'
            && w[1].is_ascii_digit()
            && w[2].is_ascii_digit()
            && (i == 0 || !bytes[i - 1].is_ascii_digit())
            && bytes.get(i + 3).is_none_or(|b| !b.is_ascii_digit())
    })
}

/// Bounded queue of reports waiting for upload, persisted as `health.json`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HealthBuffer {
    #[serde(default)]
    pub reports: VecDeque<HealthReport>,
}

impl HealthBuffer {
    pub fn record(
        &mut self,
        kind: HealthKind,
        category: HealthCategory,
        client_version: &str,
        platform: &str,
        at: i64,
        duration_ms: Option<u64>,
    ) {
        let version = sanitize_version(client_version);
        self.reports.push_back(HealthReport {
            kind,
            category,
            client_version: version,
            platform: platform.into(),
            at,
            duration_ms: duration_ms
                .filter(|d| *d <= MAX_DURATION_MS)
                .map(|d| d as u32),
        });
        while self.reports.len() > MAX_BUFFERED {
            self.reports.pop_front();
        }
    }
    pub fn batch(&self) -> Vec<HealthReport> {
        self.reports.iter().take(BATCH).cloned().collect()
    }
    pub fn drop_front(&mut self, count: usize) {
        for _ in 0..count.min(self.reports.len()) {
            self.reports.pop_front();
        }
    }
    pub fn len(&self) -> usize {
        self.reports.len()
    }
    pub fn is_empty(&self) -> bool {
        self.reports.is_empty()
    }
}

/// `^[0-9A-Za-z.+-]{1,40}$`; anything else becomes `unknown`.
pub fn sanitize_version(version: &str) -> String {
    if !version.is_empty()
        && version.len() <= 40
        && version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'+' | b'-'))
    {
        version.into()
    } else {
        "unknown".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_table() {
        for (text, expected) in [
            ("Request cancelled", C::Cancelled),
            ("write failed: No space left on device", C::StorageFull),
            (
                "java.io.FileNotFoundException: /storage/x.mp4",
                C::OfflineFileMissing,
            ),
            ("Sign in to confirm your age", C::AgeRestricted),
            (
                "This video is available to this channel's members-only",
                C::MembersOnly,
            ),
            (
                "The uploader has not made this video available in your country",
                C::GeoBlocked,
            ),
            ("Video is not available in your country", C::GeoBlocked),
            ("This video is DRM protected", C::Drm),
            ("n challenge solving failed", C::JsChallengeFailed),
            (
                "Android isolated JavaScript solver failed or timed out",
                C::JsChallengeFailed,
            ),
            ("Unable to decrypt signature cipher", C::SignatureFailed),
            ("Requested format is not available", C::FormatUnavailable),
            ("This live event will begin in 3 hours", C::LiveUnsupported),
            ("HTTP Error 429: Too Many Requests", C::Http429),
            ("HTTP Error 403: Forbidden", C::Http403),
            ("HTTP Error 404: Not Found", C::Http404),
            ("HTTP Error 503: Service Unavailable", C::Http5xx),
            ("Response code: 502 (status)", C::Http5xx),
            ("operation timed out", C::NetworkTimeout),
            ("invalid peer certificate: UnknownIssuer", C::Tls),
            ("error sending request for url", C::NetworkUnreachable),
            (
                "Unable to resolve host \"www.youtube.com\"",
                C::NetworkUnreachable,
            ),
            ("MediaCodecVideoRenderer error", C::DecoderError),
            ("AudioTrack init failed", C::AudioOutputError),
            (
                "Unable to extract initial player response",
                C::ExtractorFailed,
            ),
            ("something odd", C::Unknown),
            ("", C::Unknown),
        ] {
            assert_eq!(classify(text), expected, "{text}");
        }
        // Numbers that merely contain 5xx digits are not HTTP errors.
        assert_eq!(classify("status: 15000 frames"), C::Unknown);
    }

    #[test]
    fn buffer_is_bounded_and_batches() {
        let mut buffer = HealthBuffer::default();
        for i in 0..(MAX_BUFFERED + 7) {
            buffer.record(
                HealthKind::Start,
                C::Ok,
                "0.2.0",
                "android",
                i as i64,
                Some(10),
            );
        }
        assert_eq!(buffer.len(), MAX_BUFFERED);
        assert_eq!(buffer.reports.front().unwrap().at, 7);
        assert_eq!(buffer.batch().len(), BATCH);
        buffer.drop_front(BATCH);
        assert_eq!(buffer.len(), MAX_BUFFERED - BATCH);
        buffer.record(
            HealthKind::Stall,
            C::Ok,
            "bad version!",
            "android",
            1,
            Some(700_000),
        );
        let last = buffer.reports.back().unwrap();
        assert_eq!(last.client_version, "unknown");
        assert_eq!(last.duration_ms, None);
    }
}
