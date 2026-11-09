use super::{
    Backend, CancellationToken, Result, Stream, StreamKind, Subtitle, headers,
    hls::{Key, Playlist, Rendition, Resource},
};
use anyhow::{Context, bail, ensure};
use futures_util::{StreamExt, stream};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};
use tokio::io::AsyncWriteExt;

const RESOURCE_LIMIT: usize = 64 * 1024 * 1024;

#[derive(Clone)]
pub struct DownloadOptions {
    /// Destination directory. Must not already exist; published atomically.
    pub output: PathBuf,
    pub max_height: Option<u32>,
    /// Empty selects all advertised subtitle languages.
    pub subtitle_languages: Vec<String>,
    pub max_bytes: u64,
    /// Explicit qualification excerpt, rounded up to complete segments (1–300 s).
    pub sample_seconds: Option<f64>,
}

impl DownloadOptions {
    pub fn new(output: PathBuf) -> Self {
        Self {
            output,
            max_height: Some(1080),
            subtitle_languages: vec![],
            max_bytes: 20 * 1024 * 1024 * 1024,
            sample_seconds: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DownloadReceipt {
    pub entrypoint: PathBuf,
    pub bytes: u64,
    pub media_seconds: Option<f64>,
    pub subtitle_files: Vec<PathBuf>,
    #[serde(default)]
    pub is_excerpt: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SegmentTiming {
    pub media_seconds: f64,
    pub transfer_ms: u64,
    pub bytes: u64,
    pub buffer_seconds: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkReport {
    pub kind: StreamKind,
    pub selected_height: Option<u32>,
    pub bytes: u64,
    pub elapsed_ms: u64,
    pub media_seconds: Option<f64>,
    pub throughput_mbps: f64,
    /// Encoded media duration / total fetch time, including playlists/audio.
    pub delivery_headroom: Option<f64>,
    /// Estimate only, using a ten-second starting buffer; not decoder evidence.
    pub estimated_stalls: usize,
    pub sustainable: Option<bool>,
    pub decoder_verified: bool,
    pub segments: Vec<SegmentTiming>,
}

struct Prepared {
    video: Playlist,
    audio: Option<Playlist>,
    subtitles: Vec<Rendition>,
    height: Option<u32>,
}

enum Track<'a> {
    Media(&'a Playlist),
    Subtitles(&'a Playlist),
}

type ResourceIndex = HashMap<(url::Url, Option<(u64, u64)>, String), usize>;

#[derive(Default)]
struct Resources {
    items: Vec<(Resource, String)>,
    index: ResourceIndex,
}

impl Resources {
    fn id(&mut self, resource: &Resource, extension: &str) -> usize {
        let key = (resource.url.clone(), resource.range, extension.to_owned());
        if let Some(id) = self.index.get(&key) {
            return *id;
        }
        let id = self.items.len();
        self.items.push((resource.clone(), extension.into()));
        self.index.insert(key, id);
        id
    }
}

impl Backend {
    async fn prepare(
        &self,
        source: &Stream,
        max_height: Option<u32>,
        cancel: &CancellationToken,
    ) -> Result<Prepared> {
        source.validate()?;
        ensure!(source.kind == StreamKind::Hls, "Expected an HLS stream");
        let mut url = source.url.clone();
        let mut audio_url = None;
        let mut subtitles = vec![];
        let mut height = None;
        for _ in 0..4 {
            let playlist = self.playlist(&url, &source.headers, cancel).await?;
            if playlist.variants.is_empty() {
                let audio = match audio_url {
                    Some(url) => {
                        let audio = self.playlist(&url, &source.headers, cancel).await?;
                        ensure!(
                            audio.variants.is_empty(),
                            "Nested audio master is unsupported"
                        );
                        Some(audio)
                    }
                    None => None,
                };
                return Ok(Prepared {
                    video: playlist,
                    audio,
                    subtitles,
                    height,
                });
            }
            let variant = playlist.variant(max_height)?;
            height = variant.height.or(height);
            if let Some(group) = &variant.audio_group {
                let rendition = playlist
                    .renditions
                    .iter()
                    .filter(|r| &r.group == group && r.kind == "AUDIO")
                    .max_by_key(|r| r.default);
                ensure!(rendition.is_some(), "HLS audio group has no rendition");
                audio_url = rendition.and_then(|r| r.url.clone());
            }
            if let Some(group) = &variant.subtitle_group {
                subtitles.extend(
                    playlist
                        .renditions
                        .iter()
                        .filter(|r| &r.group == group && r.kind == "SUBTITLES")
                        .cloned(),
                );
            }
            url = variant.url.clone();
        }
        bail!("HLS master nesting exceeds limit")
    }

    async fn resource(
        &self,
        resource: &Resource,
        source: &Stream,
        limit: usize,
        cancel: &CancellationToken,
    ) -> Result<Vec<u8>> {
        for attempt in 0..3 {
            match self.resource_once(resource, source, limit, cancel).await {
                Ok(bytes) => return Ok(bytes),
                Err(error) => {
                    let transient = matches!(
                        error.to_string().as_str(),
                        "HTTP 408"
                            | "HTTP 502"
                            | "HTTP 503"
                            | "HTTP 504"
                            | "Network timeout"
                            | "Response body timeout"
                            | "Response body transfer failed"
                    );
                    if !transient || attempt == 2 {
                        return Err(error);
                    }
                    tokio::select! {
                        _ = cancel.cancelled() => bail!("Cancelled"),
                        _ = tokio::time::sleep(std::time::Duration::from_millis(100 * (attempt + 1))) => {},
                    }
                }
            }
        }
        unreachable!()
    }

    async fn resource_once(
        &self,
        resource: &Resource,
        source: &Stream,
        limit: usize,
        cancel: &CancellationToken,
    ) -> Result<Vec<u8>> {
        if let Some((_, length)) = resource.range {
            ensure!(length <= limit as u64, "Resource exceeds size limit");
        }
        let response = self
            .response(
                &resource.url,
                &headers(&source.headers)?,
                resource.range,
                cancel,
            )
            .await?;
        let bytes = self.collect(response, limit, cancel).await?;
        // A decodable HLS error card is still a provider failure. Do not publish
        // or rate its network delivery as successful movie playback.
        if self
            .blocked_assets
            .iter()
            .any(|(len, _)| *len == bytes.len())
        {
            use sha2::{Digest, Sha256};
            let digest = hex::encode(Sha256::digest(&bytes));
            ensure!(
                !self.blocked_assets.iter().any(|(_, hash)| hash == &digest),
                "Provider returned a known network-block error video (error code 4)"
            );
        }
        if let Some((_, length)) = resource.range {
            ensure!(bytes.len() as u64 == length, "Truncated ranged resource");
        }
        ensure!(!bytes.is_empty(), "Empty media resource");
        Ok(bytes)
    }

    /// Sequential real segment reads, not HEAD requests or playlist-only checks.
    /// Samples both video and a selected separate audio rendition.
    pub async fn probe(
        &self,
        source: &Stream,
        samples: usize,
        cancel: &CancellationToken,
    ) -> Result<NetworkReport> {
        ensure!((1..=100).contains(&samples), "Sample count must be 1–100");
        source.validate()?;
        let started = Instant::now();
        let mut report = NetworkReport {
            kind: source.kind.clone(),
            selected_height: None,
            bytes: 0,
            elapsed_ms: 0,
            media_seconds: None,
            throughput_mbps: 0.0,
            delivery_headroom: None,
            estimated_stalls: 0,
            sustainable: None,
            decoder_verified: false,
            segments: vec![],
        };
        match source.kind {
            StreamKind::Hls => {
                let prepared = self.prepare(source, Some(1080), cancel).await?;
                report.selected_height = prepared.height;
                let mut buffer = 10.0;
                let mut media_seconds = 0.0;
                // Measure audio as well, charging its network time against the same buffer.
                for (track, playlist) in std::iter::once(&prepared.video)
                    .chain(prepared.audio.iter())
                    .enumerate()
                {
                    let mut key = None;
                    let mut init = None;
                    let mut track_seconds = 0.0;
                    for segment in playlist.segments.iter().take(samples) {
                        let start = Instant::now();
                        if segment.key != key {
                            if let Some(key) = &segment.key {
                                let bytes =
                                    self.resource(&key.resource, source, 16, cancel).await?;
                                ensure!(bytes.len() == 16, "Invalid AES-128 key length");
                                report.bytes += bytes.len() as u64;
                            }
                            key = segment.key.clone();
                        }
                        if segment.init != init {
                            if let Some(init) = &segment.init {
                                report.bytes += self
                                    .resource(init, source, RESOURCE_LIMIT, cancel)
                                    .await?
                                    .len() as u64;
                            }
                            init = segment.init.clone();
                        }
                        let bytes = self
                            .resource(&segment.resource, source, RESOURCE_LIMIT, cancel)
                            .await?;
                        let elapsed = start.elapsed();
                        report.bytes += bytes.len() as u64;
                        track_seconds += segment.duration;
                        if track == 0 {
                            if elapsed.as_secs_f64() > buffer {
                                report.estimated_stalls += 1;
                            }
                            buffer = (buffer - elapsed.as_secs_f64()).max(0.0) + segment.duration;
                            report.segments.push(SegmentTiming {
                                media_seconds: segment.duration,
                                transfer_ms: elapsed.as_millis() as u64,
                                bytes: bytes.len() as u64,
                                buffer_seconds: buffer,
                            });
                        } else {
                            if elapsed.as_secs_f64() > buffer {
                                report.estimated_stalls += 1;
                            }
                            buffer = (buffer - elapsed.as_secs_f64()).max(0.0);
                        }
                    }
                    if media_seconds == 0.0 {
                        media_seconds = track_seconds;
                    } else {
                        media_seconds = f64::min(media_seconds, track_seconds);
                    }
                }
                report.media_seconds = Some(media_seconds);
                let headroom = media_seconds / started.elapsed().as_secs_f64().max(0.000001);
                report.delivery_headroom = Some(headroom);
                report.sustainable = Some(
                    report.segments.len() >= 3 && headroom >= 1.3 && report.estimated_stalls == 0,
                );
            }
            StreamKind::File => {
                let mut response = self
                    .response(&source.url, &headers(&source.headers)?, None, cancel)
                    .await?;
                let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(45);
                while report.bytes < 1024 * 1024 {
                    let chunk = tokio::select! {
                        _ = cancel.cancelled() => bail!("Cancelled"),
                        result = tokio::time::timeout_at(deadline, response.chunk()) => result.map_err(|_| anyhow::anyhow!("Response body timeout"))?.map_err(|_| anyhow::anyhow!("Response body transfer failed"))?,
                    };
                    let Some(chunk) = chunk else { break };
                    report.bytes += chunk.len() as u64;
                }
                ensure!(report.bytes > 0, "Empty media resource");
                // Without a media duration/bitrate this cannot qualify smooth delivery.
            }
            StreamKind::Dash => bail!("DASH probing is not implemented; no success inferred"),
        }
        report.elapsed_ms = started.elapsed().as_millis() as u64;
        report.throughput_mbps =
            report.bytes as f64 * 8.0 / started.elapsed().as_secs_f64().max(0.000001) / 1_000_000.0;
        Ok(report)
    }

    /// Complete, relocatable offline bundle. Never overwrites existing output.
    /// Partial failure/cancellation removes staging; no signed remote URLs remain.
    pub async fn download(
        &self,
        source: &Stream,
        options: &DownloadOptions,
        cancel: &CancellationToken,
    ) -> Result<DownloadReceipt> {
        source.validate()?;
        ensure!(
            options.max_bytes > 0,
            "Download byte budget must be positive"
        );
        if let Some(seconds) = options.sample_seconds {
            ensure!(
                seconds.is_finite() && (1.0..=300.0).contains(&seconds),
                "Sample duration must be 1–300 seconds"
            );
        }
        ensure!(
            !options.output.exists(),
            "Download destination already exists"
        );
        let parent = options
            .output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        tokio::fs::create_dir_all(parent).await?;
        let staging = tempfile::Builder::new()
            .prefix(".vidcue-cinecat-")
            .tempdir_in(parent)?;
        let total = Arc::new(AtomicU64::new(0));
        let mut subtitle_files = vec![];
        let (entrypoint, duration) = match source.kind {
            StreamKind::Hls => {
                let mut prepared = self.prepare(source, options.max_height, cancel).await?;
                if let Some(seconds) = options.sample_seconds {
                    excerpt(&mut prepared.video, seconds)?;
                    if let Some(audio) = &mut prepared.audio {
                        excerpt(audio, seconds)?;
                    }
                }
                let video_dir = staging.path().join("video");
                self.save_playlist(
                    Track::Media(&prepared.video),
                    source,
                    &video_dir,
                    &total,
                    options.max_bytes,
                    cancel,
                )
                .await?;
                if let Some(audio) = &prepared.audio {
                    self.save_playlist(
                        Track::Media(audio),
                        source,
                        &staging.path().join("audio"),
                        &total,
                        options.max_bytes,
                        cancel,
                    )
                    .await?;
                }
                let mut master = String::from("#EXTM3U\n#EXT-X-VERSION:7\n");
                if prepared.audio.is_some() {
                    master.push_str("#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"audio\",NAME=\"Audio\",DEFAULT=YES,AUTOSELECT=YES,URI=\"audio/index.m3u8\"\n");
                }
                let mut subtitle_count = 0;
                for rendition in &prepared.subtitles {
                    if !options.subtitle_languages.is_empty()
                        && !options.subtitle_languages.contains(&rendition.language)
                    {
                        continue;
                    }
                    let Some(url) = &rendition.url else { continue };
                    let mut playlist = self.playlist(url, &source.headers, cancel).await?;
                    if let Some(seconds) = options.sample_seconds {
                        excerpt(&mut playlist, seconds)?;
                    }
                    ensure!(
                        playlist.variants.is_empty(),
                        "Subtitle master is unsupported"
                    );
                    let folder = format!("subtitle-{subtitle_count}");
                    self.save_playlist(
                        Track::Subtitles(&playlist),
                        source,
                        &staging.path().join(&folder),
                        &total,
                        options.max_bytes,
                        cancel,
                    )
                    .await?;
                    // Do not write untrusted names into playlist syntax or filenames.
                    master.push_str(&format!("#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"subs\",NAME=\"Subtitle {subtitle_count}\",LANGUAGE=\"{}\",URI=\"{folder}/index.m3u8\"\n", safe_language(&rendition.language)));
                    subtitle_files.push(PathBuf::from(format!("{folder}/index.m3u8")));
                    subtitle_count += 1;
                }
                master.push_str(&format!(
                    "#EXT-X-STREAM-INF:BANDWIDTH=1{}{}\nvideo/index.m3u8\n",
                    if prepared.audio.is_some() {
                        ",AUDIO=\"audio\""
                    } else {
                        ""
                    },
                    if subtitle_count > 0 {
                        ",SUBTITLES=\"subs\""
                    } else {
                        ""
                    }
                ));
                tokio::fs::write(staging.path().join("index.m3u8"), master).await?;
                (
                    PathBuf::from("index.m3u8"),
                    Some(prepared.video.segments.iter().map(|s| s.duration).sum()),
                )
            }
            StreamKind::File => {
                ensure!(
                    options.sample_seconds.is_none(),
                    "Progressive excerpts require a decoder; use a complete file download"
                );
                self.save_file(
                    source,
                    &staging.path().join("video.media"),
                    &total,
                    options.max_bytes,
                    cancel,
                )
                .await?;
                (PathBuf::from("video.media"), None)
            }
            StreamKind::Dash => {
                bail!("DASH downloads are not implemented; refusing incomplete output")
            }
        };
        for (index, subtitle) in source.subtitles.iter().enumerate() {
            if !options.subtitle_languages.is_empty()
                && !options.subtitle_languages.contains(&subtitle.language)
            {
                continue;
            }
            let bytes = self.subtitle_bytes(subtitle, cancel).await?;
            charge(&total, bytes.len() as u64, options.max_bytes)?;
            let path = PathBuf::from(format!("caption-{index}.{}", subtitle_extension(&bytes)?));
            tokio::fs::write(staging.path().join(&path), bytes).await?;
            subtitle_files.push(path);
        }
        let mut receipt = DownloadReceipt {
            entrypoint: entrypoint.clone(),
            bytes: total.load(Ordering::Relaxed),
            media_seconds: duration,
            subtitle_files: subtitle_files.clone(),
            is_excerpt: options.sample_seconds.is_some(),
        };
        tokio::fs::write(
            staging.path().join("download.json"),
            serde_json::to_vec_pretty(&receipt)?,
        )
        .await?;
        ensure!(!cancel.is_cancelled(), "Cancelled");
        // persist_noclobber semantics for directories on Linux, using renameat2.
        publish(staging.path(), &options.output)?;
        receipt.entrypoint = options.output.join(entrypoint);
        receipt.subtitle_files = subtitle_files
            .into_iter()
            .map(|p| options.output.join(p))
            .collect();
        Ok(receipt)
    }

    async fn save_file(
        &self,
        source: &Stream,
        destination: &Path,
        total: &AtomicU64,
        max_bytes: u64,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let mut response = self
            .response(&source.url, &headers(&source.headers)?, None, cancel)
            .await?;
        ensure!(
            response.content_length().is_none_or(|n| n <= max_bytes),
            "Download exceeds byte budget"
        );
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .await?;
        loop {
            let chunk = tokio::select! {
                _ = cancel.cancelled() => bail!("Cancelled"),
                result = response.chunk() => result.map_err(|_| anyhow::anyhow!("Download body transfer failed"))?,
            };
            let Some(chunk) = chunk else { break };
            charge(total, chunk.len() as u64, max_bytes)?;
            file.write_all(&chunk).await?;
        }
        ensure!(total.load(Ordering::Relaxed) > 0, "Empty download");
        file.sync_all().await?;
        Ok(())
    }

    async fn save_playlist(
        &self,
        track: Track<'_>,
        source: &Stream,
        destination: &Path,
        total: &Arc<AtomicU64>,
        max_bytes: u64,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let (playlist, subtitles) = match track {
            Track::Media(p) => (p, false),
            Track::Subtitles(p) => (p, true),
        };
        ensure!(
            playlist.ended,
            "Offline download requires a finite ENDLIST playlist"
        );
        ensure!(playlist.variants.is_empty(), "Expected a media playlist");
        tokio::fs::create_dir(destination).await?;
        let mut resources = Resources::default();
        let mut body = format!(
            "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-TARGETDURATION:{}\n#EXT-X-MEDIA-SEQUENCE:{}\n",
            playlist
                .segments
                .iter()
                .map(|s| s.duration.ceil() as u64)
                .max()
                .unwrap_or(1),
            playlist.sequence
        );
        let mut last_key: Option<Key> = None;
        let mut last_init: Option<Resource> = None;
        for segment in &playlist.segments {
            if segment.discontinuity {
                body.push_str("#EXT-X-DISCONTINUITY\n");
            }
            if segment.key != last_key {
                match &segment.key {
                    Some(key) => {
                        let id = resources.id(&key.resource, "key");
                        body.push_str(&format!(
                            "#EXT-X-KEY:METHOD=AES-128,URI=\"{id}.key\"{}\n",
                            key.iv
                                .as_ref()
                                .map(|iv| format!(",IV={iv}"))
                                .unwrap_or_default()
                        ));
                    }
                    None => body.push_str("#EXT-X-KEY:METHOD=NONE\n"),
                }
                last_key = segment.key.clone();
            }
            if segment.init != last_init {
                if let Some(init) = &segment.init {
                    let id = resources.id(init, "mp4");
                    body.push_str(&format!("#EXT-X-MAP:URI=\"{id}.mp4\"\n"));
                }
                ensure!(
                    last_init.is_none() || segment.init.is_some(),
                    "Cannot clear HLS init map"
                );
                last_init = segment.init.clone();
            }
            let extension = if subtitles || segment.resource.url.path().ends_with(".vtt") {
                "vtt"
            } else if segment.init.is_some() {
                "m4s"
            } else {
                "ts"
            };
            let id = resources.id(&segment.resource, extension);
            body.push_str(&format!(
                "#EXTINF:{:.6},\n{id}.{extension}\n",
                segment.duration
            ));
        }
        body.push_str("#EXT-X-ENDLIST\n");
        // try_collect cancels in-flight requests on the first error.
        use futures_util::TryStreamExt;
        stream::iter(resources.items.into_iter().enumerate().map(
            |(id, (resource, extension))| async move {
                let is_key = extension == "key";
                let limit = if is_key { 16 } else { RESOURCE_LIMIT };
                let bytes = self.resource(&resource, source, limit, cancel).await?;
                ensure!(!is_key || bytes.len() == 16, "Invalid AES-128 key length");
                charge(total, bytes.len() as u64, max_bytes)?;
                let mut file = tokio::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(destination.join(format!("{id}.{extension}")))
                    .await?;
                file.write_all(&bytes).await?;
                file.sync_all().await?;
                Ok::<_, anyhow::Error>(())
            },
        ))
        .buffer_unordered(self.concurrency)
        .try_collect::<Vec<_>>()
        .await?;
        tokio::fs::write(destination.join("index.m3u8"), body).await?;
        Ok(())
    }

    async fn subtitle_bytes(
        &self,
        subtitle: &Subtitle,
        cancel: &CancellationToken,
    ) -> Result<Vec<u8>> {
        super::validate_url(&subtitle.url)?;
        let bytes = self
            .bytes(
                &subtitle.url,
                &reqwest::header::HeaderMap::new(),
                super::SUBTITLE_LIMIT,
                cancel,
            )
            .await?;
        subtitle_extension(&bytes)?;
        Ok(bytes)
    }

    pub async fn download_subtitle(
        &self,
        subtitle: &Subtitle,
        output: &Path,
        cancel: &CancellationToken,
    ) -> Result<u64> {
        let bytes = self.subtitle_bytes(subtitle, cancel).await?;
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        use std::io::Write;
        file.write_all(&bytes)?;
        file.as_file().sync_all()?;
        ensure!(!cancel.is_cancelled(), "Cancelled");
        file.persist_noclobber(output)
            .map_err(|_| anyhow::anyhow!("Subtitle destination exists or cannot be published"))?;
        Ok(bytes.len() as u64)
    }

    /// Publishes selected external captions together with content-detected extensions.
    pub async fn download_subtitles(
        &self,
        subtitles: &[Subtitle],
        output: &Path,
        cancel: &CancellationToken,
    ) -> Result<Vec<PathBuf>> {
        ensure!(
            !subtitles.is_empty() && subtitles.len() <= 2048,
            "Select 1–2048 subtitles"
        );
        ensure!(!output.exists(), "Subtitle destination already exists");
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let staging = tempfile::Builder::new()
            .prefix(".vidcue-captions-")
            .tempdir_in(parent)?;
        let mut paths = vec![];
        for (index, subtitle) in subtitles.iter().enumerate() {
            let bytes = self.subtitle_bytes(subtitle, cancel).await?;
            let extension = subtitle_extension(&bytes)?;
            let name = format!("{}-{index}.{extension}", safe_language(&subtitle.language));
            tokio::fs::write(staging.path().join(&name), bytes).await?;
            paths.push(output.join(name));
        }
        ensure!(!cancel.is_cancelled(), "Cancelled");
        publish(staging.path(), output)?;
        Ok(paths)
    }
}

fn excerpt(playlist: &mut Playlist, seconds: f64) -> Result<()> {
    ensure!(playlist.ended, "Excerpts require a finite VOD playlist");
    let mut total = 0.0;
    let count = playlist
        .segments
        .iter()
        .take_while(|segment| {
            if total >= seconds {
                return false;
            }
            total += segment.duration;
            true
        })
        .count();
    playlist.segments.truncate(count);
    Ok(())
}

fn charge(total: &AtomicU64, bytes: u64, limit: u64) -> Result<()> {
    total
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(bytes).filter(|n| *n <= limit)
        })
        .map_err(|_| anyhow::anyhow!("Download exceeds byte budget"))?;
    Ok(())
}

fn safe_language(language: &str) -> String {
    language
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(40)
        .collect()
}

fn subtitle_extension(bytes: &[u8]) -> Result<&'static str> {
    let text = std::str::from_utf8(bytes)
        .context("Subtitle is not UTF-8")?
        .trim_start_matches('\u{feff}')
        .trim_start();
    ensure!(!text.contains('\0'), "Invalid subtitle content");
    if text.starts_with("WEBVTT") {
        return Ok("vtt");
    }
    if text
        .lines()
        .take(8)
        .any(|line| line.contains(" --> ") && line.contains(':'))
    {
        return Ok("srt");
    }
    bail!("Subtitle is not supported UTF-8 SRT/WebVTT (compressed/HTML responses rejected)")
}

#[cfg(target_os = "linux")]
fn publish(from: &Path, to: &Path) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let from = std::ffi::CString::new(from.as_os_str().as_bytes())?;
    let to = std::ffi::CString::new(to.as_os_str().as_bytes())?;
    // SAFETY: both C strings live through the syscall; RENAME_NOREPLACE prevents clobber races.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error())
            .context("Cannot publish offline bundle without overwriting");
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn publish(_from: &Path, _to: &Path) -> Result<()> {
    // Do not silently replace a destination on platforms without this guarantee.
    bail!("Atomic offline-bundle publication currently requires Linux")
}
