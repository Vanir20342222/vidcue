//! YouTube Music lyrics for a playing song, fetched anonymously.
//!
//! Only the song's public video ID is sent, with a fresh client: no session, cookies, stream URL
//! or account context. YouTube Music lists a lyrics tab in its "next" response; its browse page
//! carries the text. The Android Music client also returns line timings when YouTube has them.
use crate::lyrics::{Line, Track};
use anyhow::{Result, ensure};
use serde_json::{Value, json};

const ORIGIN: &str = "https://music.youtube.com";
const WEB_VERSION: &str = "1.20251020.01.00";
const ANDROID_VERSION: &str = "7.27.52";
const LIMIT: usize = 4 * 1024 * 1024;

/// An 11-character YouTube video ID from a media ID or watch URL.
pub fn video_id(id: &str, url: &str) -> Option<String> {
    let valid = |v: &str| {
        v.len() == 11
            && v.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    };
    if valid(id) {
        return Some(id.to_owned());
    }
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?.trim_start_matches("www.");
    let candidate = match host {
        "youtube.com" | "music.youtube.com" | "m.youtube.com" => parsed
            .query_pairs()
            .find(|(key, _)| key == "v")
            .map(|(_, value)| value.into_owned())?,
        "youtu.be" => parsed.path().trim_start_matches('/').to_owned(),
        _ => return None,
    };
    valid(&candidate).then_some(candidate)
}

async fn post(
    client: &reqwest::Client,
    base: &str,
    endpoint: &str,
    android: bool,
    body: Value,
) -> Result<Value> {
    let mut payload = body;
    payload["context"] = if android {
        json!({"client":{"clientName":"ANDROID_MUSIC","clientVersion":ANDROID_VERSION,"androidSdkVersion":34,"hl":"en","gl":"US"}})
    } else {
        json!({"client":{"clientName":"WEB_REMIX","clientVersion":WEB_VERSION,"hl":"en","gl":"US"}})
    };
    let mut request = client
        .post(format!("{base}/youtubei/v1/{endpoint}?prettyPrint=false"))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .timeout(std::time::Duration::from_secs(10))
        .body(payload.to_string());
    request = if android {
        request.header(
            reqwest::header::USER_AGENT,
            format!("com.google.android.apps.youtube.music/{ANDROID_VERSION} (Linux; U; Android 14) gzip"),
        )
    } else {
        request.header("Origin", ORIGIN)
    };
    let response = request
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("YouTube Music lyrics unavailable: {}", e.without_url()))?;
    ensure!(
        response.status().is_success(),
        "YouTube Music lyrics HTTP {}",
        response.status().as_u16()
    );
    Ok(serde_json::from_str(
        &crate::browse::limited_text(response, LIMIT).await?,
    )?)
}

fn find<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => map
            .get(key)
            .or_else(|| map.values().find_map(|v| find(v, key))),
        Value::Array(items) => items.iter().find_map(|v| find(v, key)),
        _ => None,
    }
}

/// The lyrics page ID from a "next" response, when YouTube Music has lyrics for the song.
fn lyrics_browse_id(next: &Value) -> Option<String> {
    fn walk(value: &Value) -> Option<String> {
        match value {
            Value::Object(map) => {
                if let Some(tab) = map.get("tabRenderer") {
                    let endpoint = tab.pointer("/endpoint/browseEndpoint");
                    let page = endpoint
                        .and_then(|e| e.pointer("/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType"))
                        .and_then(Value::as_str);
                    let id = endpoint
                        .and_then(|e| e.get("browseId"))
                        .and_then(Value::as_str);
                    if let Some(id) = id
                        && (page == Some("MUSIC_PAGE_TYPE_TRACK_LYRICS") || id.starts_with("MPLY"))
                        && tab.get("unselectable").and_then(Value::as_bool) != Some(true)
                    {
                        return Some(id.to_owned());
                    }
                }
                map.values().find_map(walk)
            }
            Value::Array(items) => items.iter().find_map(walk),
            _ => None,
        }
    }
    walk(next)
}

/// Lines from the Android Music client's timed lyrics; timed when every line has a start.
fn timed_lines(data: &Value) -> Option<Vec<Line>> {
    let rows = find(data, "timedLyricsData")?.as_array()?;
    let mut lines = Vec::new();
    for row in rows.iter().take(10_000) {
        let Some(text) = row.get("lyricLine").and_then(Value::as_str) else {
            continue;
        };
        let at = row
            .pointer("/cueRange/startTimeMilliseconds")
            .and_then(|v| {
                v.as_str()
                    .and_then(|s| s.parse().ok())
                    .or_else(|| v.as_u64())
            });
        lines.push(Line {
            at_ms: at,
            text: vidcue_core::clean_text(text),
            ..Line::default()
        });
    }
    if lines.iter().any(|l| l.at_ms.is_none()) {
        for line in &mut lines {
            line.at_ms = None;
        }
    }
    lines
        .iter()
        .any(|l| !l.text.trim().is_empty())
        .then_some(lines)
}

/// Plain lines and the source credit from the web client's description shelf.
fn plain_lines(data: &Value) -> Option<(Vec<Line>, String)> {
    let shelf = find(data, "musicDescriptionShelfRenderer")?;
    let text = crate::browse::text(shelf.get("description")?);
    let lines: Vec<Line> = text
        .lines()
        .take(10_000)
        .map(|line| Line {
            at_ms: None,
            text: vidcue_core::clean_text(line),
            ..Line::default()
        })
        .collect();
    let footer = shelf
        .get("footer")
        .map(crate::browse::text)
        .unwrap_or_default();
    lines
        .iter()
        .any(|l| !l.text.trim().is_empty())
        .then_some((lines, footer))
}

async fn fetch_from(
    client: reqwest::Client,
    base: &str,
    video: &str,
    title: &str,
    artist: &str,
) -> Result<Option<Track>> {
    let next = post(&client, base, "next", false, json!({"videoId": video})).await?;
    let Some(browse) = lyrics_browse_id(&next) else {
        return Ok(None);
    };
    let track = |lines, album: String| Track {
        title: title.to_owned(),
        artist: artist.to_owned(),
        album,
        duration: 0.0,
        instrumental: false,
        lines,
    };
    // Timed lines first; a failure there still leaves the plain text.
    if let Ok(data) = post(&client, base, "browse", true, json!({"browseId": browse})).await
        && let Some(lines) = timed_lines(&data)
    {
        return Ok(Some(track(lines, "YouTube Music".into())));
    }
    let data = post(&client, base, "browse", false, json!({"browseId": browse})).await?;
    Ok(plain_lines(&data).map(|(lines, footer)| {
        track(
            lines,
            if footer.is_empty() {
                "YouTube Music".into()
            } else {
                format!("YouTube Music · {footer}")
            },
        )
    }))
}

/// YouTube Music lyrics for `video`, or `None` when the song has none.
pub async fn lookup(
    proxy: Option<&str>,
    video: &str,
    title: &str,
    artist: &str,
) -> Result<Option<Track>> {
    fetch_from(crate::http_client(proxy)?, ORIGIN, video, title, artist).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> reqwest::Client {
        reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap()
    }

    #[test]
    fn video_ids_come_from_ids_or_watch_urls_only() {
        assert_eq!(video_id("abcdefghijk", "").as_deref(), Some("abcdefghijk"));
        assert_eq!(
            video_id(
                "MPREalbum",
                "https://music.youtube.com/watch?v=a-b_c1234567&list=x"
            )
            .as_deref(),
            None
        );
        assert_eq!(
            video_id("", "https://music.youtube.com/watch?v=a-b_c123456&list=x").as_deref(),
            Some("a-b_c123456")
        );
        assert_eq!(
            video_id("", "https://youtu.be/a-b_c123456").as_deref(),
            Some("a-b_c123456")
        );
        assert_eq!(video_id("", "https://radio.test/watch?v=a-b_c123456"), None);
    }

    #[tokio::test]
    async fn timed_lyrics_are_preferred_and_only_the_video_id_is_sent() {
        let (base, server) = crate::test_http::serve_request(|request| {
            let lower = request.to_ascii_lowercase();
            assert!(!lower.contains("cookie:") && !lower.contains("authorization:"));
            let body = if lower.starts_with("post /youtubei/v1/next") {
                assert!(request.contains("\"videoId\":\"fixture0001\""));
                json!({"contents":{"tabs":[
                    {"tabRenderer":{"title":"Up next"}},
                    {"tabRenderer":{"title":"Lyrics","endpoint":{"browseEndpoint":{"browseId":"MPLYt_fixture","browseEndpointContextSupportedConfigs":{"browseEndpointContextMusicConfig":{"pageType":"MUSIC_PAGE_TYPE_TRACK_LYRICS"}}}}}}
                ]}})
            } else if request.contains("ANDROID_MUSIC") {
                assert!(request.contains("MPLYt_fixture"));
                json!({"contents":{"timedLyricsData":[
                    {"lyricLine":"First","cueRange":{"startTimeMilliseconds":"1200"}},
                    {"lyricLine":"Second","cueRange":{"startTimeMilliseconds":"3400"}}
                ]}})
            } else {
                panic!("unexpected request");
            };
            (200, String::new(), body.to_string().into_bytes())
        })
        .await;
        let track = fetch_from(client(), &base, "fixture0001", "Song", "Artist")
            .await
            .unwrap()
            .unwrap();
        assert!(track.synced());
        assert_eq!(track.lines[1].at_ms, Some(3400));
        assert_eq!(track.album, "YouTube Music");
        server.abort();
    }

    #[tokio::test]
    async fn plain_lyrics_fall_back_to_the_description_shelf() {
        let (base, server) = crate::test_http::serve_request(|request| {
            let body = if request.contains("/next") {
                json!({"tabs":[{"tabRenderer":{"endpoint":{"browseEndpoint":{"browseId":"MPLYt_plain"}}}}]})
            } else if request.contains("ANDROID_MUSIC") {
                // No timed lyrics for this song.
                json!({"contents":{}})
            } else {
                json!({"contents":[{"musicDescriptionShelfRenderer":{
                    "description":{"runs":[{"text":"Line one\nLine two"}]},
                    "footer":{"runs":[{"text":"Source: Fixture"}]}}}]})
            };
            (200, String::new(), body.to_string().into_bytes())
        })
        .await;
        let track = fetch_from(client(), &base, "fixture0002", "Song", "Artist")
            .await
            .unwrap()
            .unwrap();
        assert!(!track.synced());
        assert_eq!(track.lines.len(), 2);
        assert_eq!(track.album, "YouTube Music · Source: Fixture");
        server.abort();
    }

    #[tokio::test]
    async fn songs_without_a_lyrics_tab_return_none() {
        let (base, server) = crate::test_http::serve_request(|_| {
            (200, String::new(), br#"{"tabs":[{"tabRenderer":{"title":"Lyrics","unselectable":true,"endpoint":{"browseEndpoint":{"browseId":"MPLYt_x"}}}}]}"#.to_vec())
        })
        .await;
        assert!(
            fetch_from(client(), &base, "fixture0003", "Song", "Artist")
                .await
                .unwrap()
                .is_none()
        );
        server.abort();
    }

    #[test]
    fn untimed_rows_make_the_whole_text_plain() {
        let lines = timed_lines(&json!({"timedLyricsData":[
            {"lyricLine":"a","cueRange":{"startTimeMilliseconds":"10"}},
            {"lyricLine":"b"}
        ]}))
        .unwrap();
        assert!(lines.iter().all(|l| l.at_ms.is_none()));
    }
}
