//! Selected adaptive resources as a bounded in-memory DASH presentation.
//! No network discovery, format changes, or signed URL persistence here.
use serde_json::Value;

fn number(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| value.as_str()?.parse().ok())
}
fn range(value: &Value, size: u64) -> Option<(u64, u64)> {
    let (start, end) = (number(&value["start"])?, number(&value["end"])?);
    (start <= end && end < size && end.checked_sub(start)? < 2 * 1024 * 1024)
        .then_some((start, end))
}
fn codec(value: &Value) -> Option<&str> {
    value.as_str().filter(|v| {
        !v.is_empty()
            && *v != "none"
            && v.len() <= 128
            && v.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    })
}
fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// One validated SegmentBase resource: its media type, MIME type, duration and XML.
struct Representation {
    kind: &'static str,
    mime: String,
    millis: u64,
    xml: String,
}

fn representation(
    track: &Value,
    id: &str,
    fallback_seconds: Option<f64>,
) -> Option<Representation> {
    if track["has_drm"] == true
        || track["manifest_url"].is_string()
        || track["fragments"].as_array().is_some_and(|f| !f.is_empty())
    {
        return None;
    }
    if track["protocol"]
        .as_str()
        .is_some_and(|p| !matches!(p, "http" | "https"))
    {
        return None;
    }
    let url = track["url"].as_str()?;
    if url.chars().any(char::is_control) {
        return None;
    }
    vidcue_core::http_url(url).ok()?;
    let size = number(&track["filesize"])?;
    let init = range(&track["vidcue_init_range"], size)?;
    let segments = range(&track["vidcue_index_range"], size)?;
    if init.1 >= segments.0 {
        return None;
    }
    let video = codec(&track["vcodec"]);
    let audio = codec(&track["acodec"]);
    // SegmentBase resources are adaptive, one elementary media type each.
    let (kind, codecs) = match (video, audio) {
        (Some(codec), None) if track["acodec"] == "none" => ("video", codec),
        (None, Some(codec)) if track["vcodec"] == "none" => ("audio", codec),
        _ => return None,
    };
    let container = match track["ext"].as_str()? {
        "mp4" | "m4a" => "mp4",
        "webm" => "webm",
        _ => return None,
    };
    let millis = number(&track["vidcue_duration_ms"]).or_else(|| {
        let seconds = fallback_seconds?;
        (seconds.is_finite() && seconds > 0.0 && seconds <= 604800.0)
            .then_some((seconds * 1000.0).ceil() as u64)
    })?;
    if millis == 0 || millis > 604800000 {
        return None;
    }
    let bandwidth = track["tbr"]
        .as_f64()
        .filter(|v| v.is_finite() && *v > 0.0)
        .map(|v| (v * 1000.0).clamp(1.0, 1e9) as u64)
        .unwrap_or(1_000_000);
    let dimensions = if kind == "video" {
        match (number(&track["width"]), number(&track["height"])) {
            (Some(w), Some(h)) if w > 0 && h > 0 && w <= 16384 && h <= 16384 => {
                format!(" width=\"{w}\" height=\"{h}\"")
            }
            _ => String::new(),
        }
    } else {
        String::new()
    };
    Some(Representation {
        kind,
        mime: format!("{kind}/{container}"),
        millis,
        xml: format!(
            "<Representation id=\"{id}\" bandwidth=\"{bandwidth}\" codecs=\"{codecs}\"{dimensions}><BaseURL>{}</BaseURL><SegmentBase indexRange=\"{}-{}\"><Initialization range=\"{}-{}\"/></SegmentBase></Representation>",
            xml(url),
            segments.0,
            segments.1,
            init.0,
            init.1
        ),
    })
}

fn presentation(duration_ms: u64, body: &str) -> Option<String> {
    let result = format!(
        "<MPD xmlns=\"urn:mpeg:dash:schema:mpd:2011\" type=\"static\" mediaPresentationDuration=\"PT{}.{:03}S\" minBufferTime=\"PT0.25S\"><Period>{body}</Period></MPD>",
        duration_ms / 1000,
        duration_ms % 1000
    );
    (result.len() <= 65536).then_some(result)
}

pub(crate) fn manifest(tracks: &[&Value], fallback_seconds: Option<f64>) -> Option<String> {
    if tracks.is_empty() || tracks.len() > 2 {
        return None;
    }
    let mut body = String::new();
    let mut duration_ms = 0;
    for (index, track) in tracks.iter().enumerate() {
        let rep = representation(track, &format!("selected-{index}"), fallback_seconds)?;
        duration_ms = duration_ms.max(rep.millis);
        body.push_str(&format!(
            "<AdaptationSet id=\"{index}\" contentType=\"{}\" mimeType=\"{}\">{}</AdaptationSet>",
            rep.kind, rep.mime, rep.xml
        ));
    }
    presentation(duration_ms, &body)
}

/// Every usable H.264 MP4 rendition at or below `max_height`, with the best AAC audio, as one
/// presentation. Media3 switches between the video representations from measured bandwidth, so
/// a poor connection steps down without re-resolving. Returns None when no complete set exists;
/// the caller then keeps the single selected stream.
// The fixture example includes this module by path and never calls the ladder.
#[allow(dead_code)]
pub(crate) fn adaptive(
    data: &Value,
    max_height: u64,
    fallback_seconds: Option<f64>,
) -> Option<String> {
    // One candidate per height: the highest-bitrate H.264 rendition.
    let mut videos = std::collections::BTreeMap::<u64, (f64, &Value)>::new();
    let mut audio = Vec::new();
    for format in data["formats"].as_array()? {
        let tbr = format["tbr"]
            .as_f64()
            .filter(|v| v.is_finite())
            .unwrap_or(0.0);
        let h264 = format["vcodec"]
            .as_str()
            .is_some_and(|c| c.starts_with("avc1"));
        if h264 && format["acodec"] == "none" && format["ext"] == "mp4" {
            let height = number(&format["height"]).filter(|h| (1..=max_height).contains(h));
            if let Some(height) = height
                && videos.get(&height).is_none_or(|(best, _)| tbr > *best)
            {
                videos.insert(height, (tbr, format));
            }
        } else if format["vcodec"] == "none"
            && format["acodec"]
                .as_str()
                .is_some_and(|c| c.starts_with("mp4a"))
        {
            audio.push((tbr, format));
        }
    }
    audio.sort_by(|a, b| b.0.total_cmp(&a.0));
    let audio = audio
        .iter()
        .find_map(|(_, track)| representation(track, "audio-0", fallback_seconds))?;
    let mut duration_ms = audio.millis;
    let mut video_body = String::new();
    for (height, (_, track)) in &videos {
        let Some(rep) = representation(track, &format!("video-{height}"), fallback_seconds) else {
            continue;
        };
        duration_ms = duration_ms.max(rep.millis);
        video_body.push_str(&rep.xml);
    }
    if video_body.is_empty() {
        return None;
    }
    let body = format!(
        "<AdaptationSet id=\"0\" contentType=\"video\" mimeType=\"video/mp4\">{video_body}</AdaptationSet><AdaptationSet id=\"1\" contentType=\"{}\" mimeType=\"{}\">{}</AdaptationSet>",
        audio.kind, audio.mime, audio.xml
    );
    presentation(duration_ms, &body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn video() -> Value {
        json!({"url":"https://media.test/v?one=1&two=2","ext":"mp4","filesize":10000,"vcodec":"avc1.640028","acodec":"none","height":2160,"width":3840,"vidcue_init_range":{"start":"0","end":"700"},"vidcue_index_range":{"start":"701","end":"999"},"vidcue_duration_ms":"12345"})
    }
    fn rendition(height: u64, tbr: f64) -> Value {
        let mut v = video();
        v["url"] = format!("https://media.test/v{height}?tbr={tbr}").into();
        v["height"] = height.into();
        v["width"] = (height * 16 / 9).into();
        v["tbr"] = tbr.into();
        v
    }
    fn audio(tbr: f64) -> Value {
        let mut a = video();
        a["url"] = format!("https://media.test/a?tbr={tbr}").into();
        a["vcodec"] = "none".into();
        a["acodec"] = "mp4a.40.2".into();
        a["ext"] = "m4a".into();
        a.as_object_mut().unwrap().remove("height");
        a.as_object_mut().unwrap().remove("width");
        a["tbr"] = tbr.into();
        a
    }
    #[test]
    fn selected_pair_preserves_urls_quality_and_separate_mp4_webm_ranges() {
        let v = video();
        let mut a = video();
        a["vcodec"] = "none".into();
        a["acodec"] = "opus".into();
        a["ext"] = "webm".into();
        let m = manifest(&[&v, &a], None).unwrap();
        assert!(m.contains("height=\"2160\""));
        assert!(m.contains("video/mp4"));
        assert!(m.contains("audio/webm"));
        assert!(m.contains("one=1&amp;two=2"));
        assert!(m.contains("indexRange=\"701-999\""));
        assert!(m.contains("PT12.345S"));
        assert_eq!(m.matches("<AdaptationSet ").count(), 2);
    }
    #[test]
    fn incomplete_or_unsafe_indexes_leave_the_original_source_available() {
        for (field, value) in [
            ("filesize", json!(100)),
            ("vidcue_index_range", json!({"start":0,"end":20})),
            ("vidcue_init_range", json!({"start":0,"end":3000000})),
            ("has_drm", json!(true)),
            ("protocol", json!("m3u8_native")),
            ("acodec", json!("mp4a.40.2")),
            ("vcodec", json!("bad\"attribute")),
            ("vidcue_duration_ms", json!(0)),
        ] {
            let mut v = video();
            v[field] = value;
            assert!(manifest(&[&v], None).is_none(), "{field}");
        }
        let mut v = video();
        v.as_object_mut().unwrap().remove("vidcue_init_range");
        assert!(manifest(&[&v], Some(10.0)).is_none());
        assert!(manifest(&[], Some(10.0)).is_none());
    }
    #[test]
    fn adaptive_ladder_offers_each_capped_height_once_with_the_best_audio() {
        let data = json!({"formats": [
            rendition(240, 300.0),
            rendition(720, 1200.0),
            rendition(720, 2400.0),
            rendition(1080, 4000.0),
            rendition(1440, 9000.0),
            audio(128.0),
            audio(48.0),
        ]});
        let m = adaptive(&data, 1080, None).unwrap();
        assert_eq!(m.matches("<Representation ").count(), 4);
        assert_eq!(m.matches("<AdaptationSet ").count(), 2);
        assert!(m.contains("id=\"video-240\""));
        assert!(m.contains("id=\"video-720\""));
        assert!(m.contains("id=\"video-1080\""));
        assert!(!m.contains("1440"));
        assert!(m.contains("bandwidth=\"2400000\""));
        assert!(!m.contains("bandwidth=\"1200000\""));
        assert!(m.contains("bandwidth=\"128000\""));
        assert!(m.contains("contentType=\"audio\""));
        assert!(m.contains("PT12.345S"));
    }
    #[test]
    fn adaptive_ladder_needs_audio_and_video_and_skips_unindexed_renditions() {
        let video_only = json!({"formats": [rendition(480, 800.0)]});
        assert!(adaptive(&video_only, 1080, None).is_none());
        let audio_only = json!({"formats": [audio(128.0)]});
        assert!(adaptive(&audio_only, 1080, None).is_none());
        let mut unindexed = rendition(360, 500.0);
        unindexed
            .as_object_mut()
            .unwrap()
            .remove("vidcue_index_range");
        let mixed = json!({"formats": [unindexed, rendition(720, 1500.0), audio(128.0)]});
        let m = adaptive(&mixed, 1080, None).unwrap();
        assert!(!m.contains("video-360"));
        assert!(m.contains("video-720"));
        let none_usable = json!({"formats": [rendition(480, 800.0), audio(128.0)]});
        assert!(adaptive(&none_usable, 240, None).is_none());
    }
}
