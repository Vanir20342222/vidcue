//! Movies (alpha) engine operations for Android.
//!
//! The Android Movies page plays and downloads openly licensed films and public test streams
//! through the shared Cinecat transfer code (`vidcue_providers::cinecat`): HLS parsing, quality
//! choice, separate audio and subtitle renditions, segment probing and atomic offline bundles.
//! Sources come from the app's built-in catalog or a URL the user typed. Cinecat titles add
//! `movie_cinecat_*` operations, which resolve through the Cinecat sources API with provider
//! failover and hand the winning streams back in the app's source format.

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf, time::Instant};
use tokio_util::sync::CancellationToken;
use url::Url;
use vidcue_providers::cinecat::{
    Backend, DownloadOptions, Stream, StreamKind, Subtitle, WebSources,
};

/// The transfer code lives on `Backend`; its API base is never contacted by these operations.
const UNUSED_API_BASE: &str = "https://movies.invalid";
/// At most this many sources are tried for one title.
const MAX_SOURCES: usize = 8;
/// Probe samples per source: enough to read real segments without a long wait.
const PROBE_SAMPLES: usize = 2;

/// The Cinecat sources API: the same defaults the standalone CLI ships, so resolution behaves
/// identically to `vidcue-cinecat` on this network (ZCDN web adapters first, then the API list).
const CINECAT_API: &str = "https://edge.cinecat.eu";
const ZCDN_PRIMARY: &str = "https://zcdn2.2wwlgkfo-31a.workers.dev";
const ZCDN_BACKUP_1: &str = "https://zcdn1.2wwlgkfo-31a.workers.dev";
const ZCDN_BACKUP_3: &str = "https://zcdn3.2wwlgkfo-31a.workers.dev";
const APHRODITE_CDN: &str = "https://cdn.hls.lol";

#[derive(Deserialize)]
struct SourceInput {
    kind: String,
    url: String,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    #[serde(default)]
    subtitles: Vec<Subtitle>,
}

fn backend() -> Result<Backend> {
    Backend::new(Url::parse(UNUSED_API_BASE)?, 4)
}

/// The backend that talks to the Cinecat sources API: web providers first (the only route that
/// serves media on this network), then every advertised API provider.
fn cinecat_backend() -> Result<Backend> {
    let sources = WebSources::new(Url::parse(ZCDN_PRIMARY)?, Url::parse(APHRODITE_CDN)?)?
        .with_zcdn_backup("zcdn1", Url::parse(ZCDN_BACKUP_1)?)?
        .with_zcdn_backup("zcdn3", Url::parse(ZCDN_BACKUP_3)?)?;
    Ok(Backend::new(Url::parse(CINECAT_API)?, 2)?.with_web_sources(sources))
}

/// Parses the media request: a TMDB id plus season/episode for TV. A malformed (negative)
/// episode is rejected rather than silently resolved as a movie.
fn media_of(r: &Value) -> Result<vidcue_providers::cinecat::Media> {
    let tmdb = r["tmdb"].as_u64().context("Missing TMDB id")?;
    let season = r["season"]
        .as_i64()
        .map(|s| u32::try_from(s).context("Invalid season"))
        .transpose()?;
    let episode = r["episode"]
        .as_i64()
        .map(|e| u32::try_from(e).context("Invalid episode"))
        .transpose()?;
    let media = vidcue_providers::cinecat::Media {
        tmdb,
        season,
        episode,
    };
    media.validate()?;
    Ok(media)
}

/// Optional explicit failover order; absent means the backend's own default order.
fn failover_order(r: &Value) -> Result<Option<Vec<String>>> {
    r["order"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|v| Ok(v.as_str().context("Invalid failover order")?.to_string()))
                .collect::<Result<Vec<_>>>()
        })
        .transpose()
}

fn stream_of(source: &SourceInput) -> Result<Stream> {
    let kind = match source.kind.as_str() {
        "hls" => StreamKind::Hls,
        "file" => StreamKind::File,
        _ => anyhow::bail!("Unsupported source type"),
    };
    let url = Url::parse(source.url.trim()).map_err(|_| anyhow::anyhow!("Invalid source URL"))?;
    let stream = Stream {
        kind,
        url,
        headers: source.headers.clone(),
        subtitles: source.subtitles.clone(),
    };
    stream.validate()?;
    Ok(stream)
}

/// One resolved stream in the app's source format, so play and download reuse the existing paths.
/// Dash is skipped: the player and the transfer code only handle HLS and progressive files.
fn source_json(provider: &str, stream: &Stream) -> Option<Value> {
    let (kind, kind_label) = match stream.kind {
        StreamKind::Hls => ("hls", "HLS"),
        StreamKind::File => ("file", "MP4"),
        StreamKind::Dash => return None,
    };
    let subs = stream.subtitles.len();
    let subs_label = match subs {
        0 => String::new(),
        1 => ", 1 subtitle".to_string(),
        n => format!(", {n} subtitles"),
    };
    let label = format!("{provider} ({kind_label}{subs_label})");
    Some(json!({
        "label": label,
        "kind": kind,
        "url": stream.url.as_str(),
        "headers": stream.headers,
        "subtitles": stream.subtitles,
    }))
}

/// `movie_check`: tries each source in order with real segment reads and stops at the first that
/// delivers media (failover). Every attempt is reported, so the page can show what each did.
/// A malformed source entry fails that attempt instead of the whole request.
pub async fn check(r: &Value, cancel: &CancellationToken) -> Result<Value> {
    let sources = r["sources"].as_array().context("Missing sources")?;
    ensure!(!sources.is_empty(), "No sources to check");
    ensure!(sources.len() <= MAX_SOURCES, "Too many sources");
    let all = r["all"].as_bool().unwrap_or(false);
    let backend = backend()?;
    let mut attempts = vec![];
    let mut chosen = Value::Null;
    for (index, source) in sources.iter().enumerate() {
        let started = Instant::now();
        let label = source["label"].as_str().unwrap_or("").to_string();
        let outcome = match serde_json::from_value::<SourceInput>(source.clone())
            .context("Invalid source entry")
        {
            Ok(source) => match stream_of(&source) {
                Ok(stream) => backend.probe(&stream, PROBE_SAMPLES, cancel).await,
                Err(error) => Err(error),
            },
            Err(error) => Err(error),
        };
        let elapsed_ms = started.elapsed().as_millis() as u64;
        match outcome {
            Ok(report) => {
                attempts.push(json!({
                    "index": index,
                    "label": label,
                    "ok": true,
                    "elapsed_ms": elapsed_ms,
                    "height": report.selected_height,
                    "throughput_mbps": (report.throughput_mbps * 10.0).round() / 10.0,
                }));
                if chosen.is_null() {
                    chosen = json!(index);
                    if !all {
                        break;
                    }
                }
            }
            Err(error) => {
                if cancel.is_cancelled() {
                    anyhow::bail!("Request cancelled");
                }
                attempts.push(json!({
                    "index": index,
                    "label": label,
                    "ok": false,
                    "elapsed_ms": elapsed_ms,
                    // Errors from the transfer code name statuses, never URLs.
                    "error": format!("{error:#}"),
                }))
            }
        }
    }
    Ok(json!({ "chosen": chosen, "attempts": attempts }))
}

/// `movie_download`: an offline bundle in `output` (which must not exist yet). HLS keeps the
/// chosen quality, one audio rendition and every subtitle rendition as a local playlist; a file
/// source is saved as one file. Cancelling drops the private staging folder.
pub async fn download(r: &Value, cancel: &CancellationToken) -> Result<Value> {
    let source: SourceInput =
        serde_json::from_value(r["source"].clone()).context("Missing source")?;
    let stream = stream_of(&source)?;
    let output = PathBuf::from(r["output"].as_str().context("Missing output folder")?);
    ensure!(output.is_absolute(), "Output folder must be absolute");
    let mut options = DownloadOptions::new(output);
    options.max_height = match r["max_height"].as_u64() {
        Some(0) => None,
        Some(height) => Some(u32::try_from(height).context("Invalid height")?),
        None => Some(1080),
    };
    let backend = backend()?;
    let receipt = match backend.download(&stream, &options, cancel).await {
        // Every rendition is taller than the cap: take the stream as published instead.
        Err(error)
            if options.max_height.is_some()
                && format!("{error}").contains("No HLS variant fits") =>
        {
            options.max_height = None;
            backend.download(&stream, &options, cancel).await?
        }
        result => result?,
    };
    Ok(json!({
        "entrypoint": receipt.entrypoint,
        "bytes": receipt.bytes,
        "media_seconds": receipt.media_seconds,
        "subtitles": receipt.subtitle_files.len(),
        "kind": source.kind,
    }))
}

/// `movie_cinecat_resolve`: asks the Cinecat sources API which provider serves a TMDB id, with
/// automatic failover. The winner's streams come back in the app's source format (label, kind,
/// url, headers, subtitles) so play and download reuse the existing paths, together with every
/// provider attempt that came before the win.
pub async fn cinecat_resolve(r: &Value, cancel: &CancellationToken) -> Result<Value> {
    let media = media_of(r)?;
    let order = failover_order(r)?;
    let verify = r["verify"].as_bool().unwrap_or(true);
    let backend = cinecat_backend()?;
    let auto = backend
        .resolve_auto(&media, order.as_deref(), verify, cancel)
        .await?;
    let sources = auto
        .resolution
        .streams
        .iter()
        .filter_map(|s| source_json(&auto.resolution.provider, s))
        .collect::<Vec<_>>();
    let attempts = auto
        .attempts
        .iter()
        .map(|a| {
            json!({
                "provider": a.provider,
                "ok": a.succeeded,
                "stream_count": if a.succeeded { auto.resolution.streams.len() } else { 0 },
                "elapsed_ms": a.elapsed_ms,
                "error": a.error,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "provider": auto.resolution.provider,
        "sources": sources,
        "attempts": attempts,
        "elapsed_ms": auto.resolution.elapsed_ms,
    }))
}

/// `movie_cinecat_check_all`: asks every Cinecat provider for the media and probes samples, so
/// the page can show how each source did. One provider's success never masks the others.
pub async fn cinecat_check_all(r: &Value, cancel: &CancellationToken) -> Result<Value> {
    let media = media_of(r)?;
    let segments = r["segments"].as_u64().unwrap_or(2);
    ensure!((1..=100).contains(&segments), "Sample count must be 1–100");
    let segments = usize::try_from(segments).context("Invalid sample count")?;
    let backend = cinecat_backend()?;
    let reports = backend.check_all(&media, segments, cancel).await?;
    Ok(json!({"reports":
        reports
            .iter()
            .map(|report| {
                json!({
                    "provider": report.provider,
                    "ok": report.resolved,
                    "stream_count": report.stream_count,
                    "elapsed_ms": report.elapsed_ms,
                    "error": report.error,
                })
            })
            .collect::<Vec<_>>()
    }))
}

/// `movie_cinecat_providers`: the providers the Cinecat sources API and the web adapters offer.
pub async fn cinecat_providers(_r: &Value, cancel: &CancellationToken) -> Result<Value> {
    let backend = cinecat_backend()?;
    let list = backend.providers(cancel).await?;
    Ok(json!(
        list.iter()
            .map(|p| json!({"name": p.name, "group": p.group}))
            .collect::<Vec<_>>()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsupported_or_unsafe_sources() {
        let bad = |kind: &str, url: &str| {
            stream_of(&SourceInput {
                kind: kind.into(),
                url: url.into(),
                headers: Default::default(),
                subtitles: vec![],
            })
            .is_err()
        };
        assert!(bad("dash", "https://a.example/x.mpd"));
        assert!(bad("hls", "file:///etc/passwd"));
        assert!(bad("hls", "https://user:pw@a.example/x.m3u8"));
        assert!(bad("file", "nonsense"));
        assert!(!bad("file", "https://a.example/film.mp4"));
    }

    #[test]
    fn resolved_sources_carry_the_provider_headers_and_subtitles() {
        let stream = Stream {
            kind: StreamKind::Hls,
            url: Url::parse("https://zcdn.example/master.m3u8").unwrap(),
            headers: [("Origin".into(), "https://cinecat.eu".into())].into(),
            subtitles: vec![Subtitle {
                url: Url::parse("https://zcdn.example/en.vtt").unwrap(),
                language: "en".into(),
                display: "English".into(),
                format: "vtt".into(),
            }],
        };
        let source = source_json("zcdn", &stream).unwrap();
        assert_eq!(source["label"], "zcdn (HLS, 1 subtitle)");
        assert_eq!(source["kind"], "hls");
        assert_eq!(source["url"], "https://zcdn.example/master.m3u8");
        assert_eq!(source["headers"]["Origin"], "https://cinecat.eu");
        assert_eq!(source["subtitles"][0]["format"], "vtt");
        // Round-trips back into the checker/downloader input with the capabilities intact.
        let input: SourceInput = serde_json::from_value(source).unwrap();
        let rebuilt = stream_of(&input).unwrap();
        assert_eq!(
            rebuilt.headers.get("Origin").map(String::as_str),
            Some("https://cinecat.eu")
        );
        assert_eq!(rebuilt.subtitles.len(), 1);
    }

    #[test]
    fn resolved_dash_streams_are_skipped_for_the_player() {
        let stream = Stream {
            kind: StreamKind::Dash,
            url: Url::parse("https://a.example/x.mpd").unwrap(),
            headers: Default::default(),
            subtitles: vec![],
        };
        assert!(source_json("zcdn", &stream).is_none());
    }

    #[tokio::test]
    async fn cinecat_media_requests_validate_their_input() {
        let cancel = CancellationToken::new();
        assert!(cinecat_resolve(&json!({}), &cancel).await.is_err());
        assert!(cinecat_resolve(&json!({"tmdb": 0}), &cancel).await.is_err());
        assert!(
            cinecat_resolve(&json!({"tmdb": 1, "season": 1}), &cancel)
                .await
                .is_err()
        );
        assert!(
            cinecat_check_all(&json!({"tmdb": 1, "segments": 0}), &cancel)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn failover_orders_must_be_names() {
        let cancel = CancellationToken::new();
        // A malformed order fails before any network work; the backend re-validates the names.
        let bad = cinecat_resolve(
            &json!({"tmdb": 45745, "order": ["zcdn", "not a provider!"]}),
            &cancel,
        )
        .await;
        assert!(bad.is_err());
    }

    #[tokio::test]
    async fn check_reports_every_failed_source_without_network() {
        let r = json!({"sources": [
            {"label": "A", "kind": "dash", "url": "https://a.example/x.mpd"},
            {"label": "B", "kind": "hls", "url": "ftp://a.example/x.m3u8"},
        ]});
        let result = check(&r, &CancellationToken::new()).await.unwrap();
        assert!(result["chosen"].is_null());
        let attempts = result["attempts"].as_array().unwrap();
        assert_eq!(attempts.len(), 2);
        assert!(attempts.iter().all(|a| a["ok"] == false));
        assert_eq!(attempts[1]["label"], "B");
    }

    /// A malformed entry fails that attempt instead of the whole request.
    #[tokio::test]
    async fn check_reports_a_malformed_source_without_failing_the_request() {
        let r = json!({"sources": [
            {"label": "Bad", "kind": "hls"},
            {"label": "Good", "kind": "hls", "url": "https://a.example/x.m3u8"},
        ]});
        let result = check(&r, &CancellationToken::new()).await.unwrap();
        let attempts = result["attempts"].as_array().unwrap();
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0]["label"], "Bad");
        assert_eq!(attempts[0]["ok"], false);
        assert_eq!(attempts[1]["label"], "Good");
    }

    /// A negative season must not silently resolve the request as a movie.
    #[tokio::test]
    async fn media_requests_reject_negative_seasons() {
        let cancel = CancellationToken::new();
        assert!(
            cinecat_resolve(&json!({"tmdb": 1396, "season": -1, "episode": 1}), &cancel)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn check_and_download_validate_their_input() {
        let cancel = CancellationToken::new();
        assert!(check(&json!({"sources": []}), &cancel).await.is_err());
        let many: Vec<Value> = (0..9)
            .map(|_| json!({"kind": "hls", "url": "https://a.example/x.m3u8"}))
            .collect();
        assert!(check(&json!({ "sources": many }), &cancel).await.is_err());
        let source = json!({"kind": "file", "url": "https://a.example/x.mp4"});
        assert!(
            download(&json!({"source": source, "output": "relative"}), &cancel)
                .await
                .is_err()
        );
    }

    /// Live check against public open-movie test streams; run with `--ignored` when online.
    #[tokio::test]
    #[ignore]
    async fn live_open_streams_fail_over_to_a_working_source() {
        let r = json!({"sources": [
            {"label": "Broken", "kind": "hls", "url": "https://test-streams.mux.dev/does-not-exist/x.m3u8"},
            {"label": "Mux", "kind": "hls", "url": "https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8"},
        ]});
        let result = check(&r, &CancellationToken::new()).await.unwrap();
        assert_eq!(result["chosen"], 1, "{result}");
        assert_eq!(result["attempts"][0]["ok"], false);
    }

    /// Live Cinecat resolution against the sources API; run with `--ignored` when online.
    /// Mirrors the standalone CLI's live evidence: ZCDN serves Dune (TMDB 438631).
    #[tokio::test]
    #[ignore]
    async fn live_cinecat_resolve_serves_a_pool_title_with_failover_attempts() {
        let result = cinecat_resolve(&json!({"tmdb": 438631}), &CancellationToken::new())
            .await
            .unwrap();
        let sources = result["sources"].as_array().unwrap();
        assert!(!sources.is_empty(), "{result}");
        assert!(
            sources
                .iter()
                .all(|s| s["kind"] == "hls" || s["kind"] == "file"),
            "{result}"
        );
        let attempts = result["attempts"].as_array().unwrap();
        assert!(!attempts.is_empty(), "{result}");
        // At least one provider won; its attempt carries the stream count.
        let winner = result["provider"].as_str().unwrap();
        let won = attempts
            .iter()
            .any(|a| a["provider"] == winner && a["ok"] == true);
        assert!(won, "{result}");
    }
}
