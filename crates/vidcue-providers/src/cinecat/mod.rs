//! Independent Cinecat sources API and native web-provider adapters. No Android integration.
//!
//! Resolved URLs are short lived capabilities: do not log or persist them in reports.
//! Native web adapters implement observed public HTTP contracts without executing site code.
mod hls;
mod transfer;
mod web;

use anyhow::{Context, Result, bail, ensure};
use futures_util::{StreamExt, stream};
use reqwest::{Client, Response, StatusCode, header::HeaderMap};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Duration};
use tokio_util::sync::CancellationToken;
use url::Url;

pub use hls::{Playlist, Rendition, Segment, Variant};
pub use transfer::{DownloadOptions, DownloadReceipt, NetworkReport, SegmentTiming};
pub use web::WebSources;

const JSON_LIMIT: usize = 2 * 1024 * 1024;
const PLAYLIST_LIMIT: usize = 2 * 1024 * 1024;
const SUBTITLE_LIMIT: usize = 8 * 1024 * 1024;

#[derive(Clone)]
pub struct Backend {
    client: Client,
    base: Url,
    concurrency: usize,
    web: Option<WebSources>,
    blocked_assets: Vec<(usize, String)>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Provider {
    pub name: String,
    #[serde(default)]
    pub group: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Media {
    pub tmdb: u64,
    #[serde(default)]
    pub season: Option<u32>,
    #[serde(default)]
    pub episode: Option<u32>,
}

impl Media {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.tmdb > 0, "TMDB ID must be positive");
        ensure!(
            matches!(
                (self.season, self.episode),
                (None, None) | (Some(_), Some(1..))
            ),
            "TV requests need both season and a positive episode"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StreamKind {
    Hls,
    #[serde(alias = "mp4", alias = "progressive")]
    File,
    Dash,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Subtitle {
    pub url: Url,
    #[serde(default = "unknown_language", alias = "lang")]
    pub language: String,
    #[serde(default, alias = "label", alias = "name")]
    pub display: String,
    #[serde(default, alias = "type")]
    pub format: String,
}
fn unknown_language() -> String {
    "und".into()
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Stream {
    pub kind: StreamKind,
    pub url: Url,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default, alias = "captions")]
    pub subtitles: Vec<Subtitle>,
}

// Deliberately omit Debug on capability-bearing Stream/Subtitle.
#[derive(Clone, Serialize, Deserialize)]
pub struct Resolution {
    pub provider: String,
    pub streams: Vec<Stream>,
    pub elapsed_ms: u64,
}

/// One candidate tried during automatic failover. Errors are sanitized.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FailoverAttempt {
    pub provider: String,
    pub succeeded: bool,
    pub elapsed_ms: u64,
    pub error: Option<String>,
}

/// The winning resolution plus every attempt that came before it.
#[derive(Clone, Serialize)]
pub struct AutoResolution {
    pub resolution: Resolution,
    pub attempts: Vec<FailoverAttempt>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderReport {
    pub provider: String,
    pub resolved: bool,
    pub elapsed_ms: u64,
    pub stream_count: usize,
    pub error: Option<String>,
    pub network: Vec<NetworkReport>,
}

pub(crate) fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 40
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
}

pub(crate) fn validate_url(url: &Url) -> Result<()> {
    ensure!(
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none(),
        "Expected an HTTP(S) URL without credentials or fragment"
    );
    Ok(())
}

pub(crate) fn headers(values: &BTreeMap<String, String>) -> Result<HeaderMap> {
    let mut result = HeaderMap::new();
    for (name, value) in values {
        // Never forward cookies/auth supplied by an untrusted API across CDNs.
        ensure!(
            matches!(
                name.to_ascii_lowercase().as_str(),
                "user-agent" | "referer" | "origin" | "accept" | "accept-language"
            ),
            "Unsupported stream header"
        );
        result.insert(
            reqwest::header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| anyhow::anyhow!("Invalid stream header name"))?,
            value
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid stream header value"))?,
        );
    }
    Ok(result)
}

impl Backend {
    pub fn new(base: Url, concurrency: usize) -> Result<Self> {
        validate_url(&base)?;
        ensure!(base.query().is_none(), "API base cannot have a query");
        ensure!((1..=8).contains(&concurrency), "Concurrency must be 1–8");
        let client = Client::builder()
            .user_agent("Vidcue-Cinecat/0.1")
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(20))
            .pool_idle_timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let base = Url::parse(&format!("{}/", base.as_str().trim_end_matches('/')))?;
        Ok(Self {
            client,
            base,
            concurrency,
            web: None,
            // Observed decoded Atlantic "network blocked / error code 4" card.
            // Hashes identify error assets, not movies, URLs or private capabilities.
            blocked_assets: [
                (
                    63356,
                    "1d3fa7f8f1f5f7728fd2c613072ef47b5e1f278559321aa43914bc8dba4cbc3d",
                ),
                (
                    64484,
                    "e3fba4af4ab730303cd60dfca465a198439d75c5103afe9fb0b1e15c8079c8cf",
                ),
                (
                    64484,
                    "d34259874252c132c0b8c6a57da999d522aaa47e6a51a5305791e32f4d8a5254",
                ),
                (
                    64484,
                    "659e5ffa06c34aef959f4656cfd589486779333ff80012a5c4024d8410ff5818",
                ),
            ]
            .into_iter()
            .map(|(len, hash)| (len, hash.into()))
            .collect(),
        })
    }

    /// Adds Cinecat's two ordinary web providers, independently of the sources API.
    pub fn with_web_sources(mut self, sources: WebSources) -> Self {
        self.web = Some(sources);
        self
    }

    /// Enumerates configured web providers and every server-advertised API provider.
    /// Does not silently substitute a cached list on failure.
    pub async fn providers(&self, cancel: &CancellationToken) -> Result<Vec<Provider>> {
        #[derive(Deserialize)]
        struct Catalog {
            sources: Vec<Provider>,
        }
        let url = self.base.join("sources")?;
        let bytes = self
            .bytes(&url, &HeaderMap::new(), JSON_LIMIT, cancel)
            .await?;
        let mut catalog: Catalog = serde_json::from_slice(&bytes)
            .map_err(|_| anyhow::anyhow!("Invalid sources catalog"))?;
        ensure!(
            catalog.sources.len() <= 256,
            "Provider catalog exceeds limit"
        );
        let mut seen = std::collections::HashSet::new();
        for p in &catalog.sources {
            ensure!(valid_name(&p.name), "Invalid provider identifier");
            ensure!(seen.insert(&p.name), "Duplicate provider identifier");
        }
        ensure!(!catalog.sources.is_empty(), "Provider catalog is empty");
        if let Some(web) = &self.web {
            let names = web.provider_names();
            ensure!(
                !catalog
                    .sources
                    .iter()
                    .any(|p| names.iter().any(|n| n == &p.name)),
                "Provider catalog collides with web provider identifiers"
            );
            catalog.sources.splice(
                0..0,
                names
                    .into_iter()
                    .map(|name| Provider {
                        name,
                        group: Some("web".into()),
                    })
                    .collect::<Vec<_>>(),
            );
        }
        Ok(catalog.sources)
    }

    pub async fn resolve(
        &self,
        provider: &str,
        media: &Media,
        cancel: &CancellationToken,
    ) -> Result<Resolution> {
        ensure!(valid_name(provider), "Invalid provider identifier");
        media.validate()?;
        if let Some(web) = &self.web
            && web.provider_names().iter().any(|name| name == provider)
        {
            return self.resolve_web(provider, media, cancel).await;
        }
        let started = std::time::Instant::now();
        let mut url = self.base.join(&format!("sources/{provider}"))?;
        url.query_pairs_mut()
            .append_pair("tmdb", &media.tmdb.to_string())
            .append_pair(
                "type",
                if media.season.is_some() {
                    "tv"
                } else {
                    "movie"
                },
            );
        if let (Some(season), Some(episode)) = (media.season, media.episode) {
            url.query_pairs_mut()
                .append_pair("season", &season.to_string())
                .append_pair("episode", &episode.to_string());
        }
        let bytes = self
            .bytes(&url, &HeaderMap::new(), JSON_LIMIT, cancel)
            .await?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|_| anyhow::anyhow!("Invalid source response"))?;
        let entries = value["streams"]
            .as_array()
            .context("Source response has no streams array")?;
        ensure!(entries.len() <= 64, "Source stream count exceeds limit");
        let mut streams = Vec::new();
        for entry in entries {
            // Unknown future kinds fail explicitly rather than claiming full coverage.
            let mut entry = entry.clone();
            let raw = entry["url"].as_str().context("Stream has no URL")?;
            let absolute = self
                .base
                .join(raw)
                .map_err(|_| anyhow::anyhow!("Invalid stream URL"))?;
            entry["url"] = absolute.as_str().into();
            let stream: Stream = serde_json::from_value(entry)
                .map_err(|_| anyhow::anyhow!("Unsupported or malformed stream response"))?;
            stream.validate()?;
            streams.push(stream);
        }
        ensure!(!streams.is_empty(), "Provider returned no streams");
        Ok(Resolution {
            provider: provider.into(),
            streams,
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    }

    /// Tries providers in order until one serves the media: the first candidate
    /// is the default and the rest are backups. With no explicit order the
    /// default is the configured web providers followed by every advertised API
    /// provider. When `verify` is set, a candidate only wins if its streams
    /// actually deliver media (the known error-video asset and dead manifests
    /// therefore fail over instead of being published as success).
    pub async fn resolve_auto(
        &self,
        media: &Media,
        order: Option<&[String]>,
        verify: bool,
        cancel: &CancellationToken,
    ) -> Result<AutoResolution> {
        media.validate()?;
        let candidates: Vec<String> = match order {
            Some(order) => {
                ensure!(!order.is_empty(), "Failover order cannot be empty");
                ensure!(order.len() <= 64, "Failover order exceeds limit");
                let mut seen = std::collections::HashSet::new();
                let mut names = Vec::new();
                for name in order {
                    ensure!(valid_name(name), "Invalid provider identifier");
                    ensure!(
                        seen.insert(name.as_str()),
                        "Duplicate provider in failover order"
                    );
                    names.push(name.clone());
                }
                names
            }
            None => Vec::new(),
        };
        let mut attempts = Vec::new();
        let mut tried = std::collections::HashSet::new();
        // Defaults: the configured web providers first, then every advertised API
        // provider. The catalog is only fetched once the web candidates are
        // exhausted, so a healthy web provider never waits on an API listing.
        let mut stage: usize = 0;
        loop {
            let round: Vec<String> = if !candidates.is_empty() {
                if stage > 0 {
                    break;
                }
                candidates.clone()
            } else if stage == 0 && self.web.is_some() {
                self.web.as_ref().expect("checked above").provider_names()
            } else {
                if stage > 1 || (stage == 1 && !tried.contains("zcdn")) {
                    break;
                }
                match self.providers(cancel).await {
                    Ok(providers) => providers
                        .into_iter()
                        .map(|p| p.name)
                        .filter(|name| !tried.contains(name.as_str()))
                        .collect(),
                    Err(error) => {
                        attempts.push(FailoverAttempt {
                            provider: "sources-catalog".into(),
                            succeeded: false,
                            elapsed_ms: 0,
                            error: Some(error.to_string()),
                        });
                        break;
                    }
                }
            };
            stage += 1;
            for provider in &round {
                tried.insert(provider.clone());
                let started = std::time::Instant::now();
                let mut attempt = FailoverAttempt {
                    provider: provider.clone(),
                    succeeded: false,
                    elapsed_ms: 0,
                    error: None,
                };
                let resolution = match self.resolve(provider, media, cancel).await {
                    Ok(resolution) => resolution,
                    Err(error) => {
                        attempt.error = Some(error.to_string());
                        attempt.elapsed_ms = started.elapsed().as_millis() as u64;
                        attempts.push(attempt);
                        continue;
                    }
                };
                if verify {
                    let mut failure = None;
                    for stream in &resolution.streams {
                        // Three segments match the sustainable-delivery sample size;
                        // any blocked or non-delivering stream disqualifies the provider.
                        if let Err(error) = self.probe(stream, 3, cancel).await {
                            failure = Some(error.to_string());
                            break;
                        }
                    }
                    if let Some(error) = failure {
                        attempt.error = Some(error);
                        attempt.elapsed_ms = started.elapsed().as_millis() as u64;
                        attempts.push(attempt);
                        continue;
                    }
                }
                attempt.succeeded = true;
                attempt.elapsed_ms = started.elapsed().as_millis() as u64;
                attempts.push(attempt);
                return Ok(AutoResolution {
                    resolution,
                    attempts,
                });
            }
        }
        let detail = attempts
            .iter()
            .map(|a| match &a.error {
                Some(error) => format!("{}: {error}", a.provider),
                None => format!("{}: no error recorded", a.provider),
            })
            .collect::<Vec<_>>()
            .join("; ");
        bail!(
            "No provider served the requested media; {} candidate(s) failed: {detail}",
            attempts.len()
        );
    }

    /// Each provider is checked independently: one success never masks others.
    pub async fn check_all(
        &self,
        media: &Media,
        segments: usize,
        cancel: &CancellationToken,
    ) -> Result<Vec<ProviderReport>> {
        media.validate()?;
        ensure!((1..=100).contains(&segments), "Sample count must be 1–100");
        let providers = self.providers(cancel).await?;
        let reports = stream::iter(providers.into_iter().map(|provider| async move {
            let start = std::time::Instant::now();
            let mut report = ProviderReport {
                provider: provider.name.clone(),
                resolved: false,
                elapsed_ms: 0,
                stream_count: 0,
                error: None,
                network: vec![],
            };
            match self.resolve(&provider.name, media, cancel).await {
                Err(error) => report.error = Some(error.to_string()),
                Ok(resolution) => {
                    report.resolved = true;
                    report.stream_count = resolution.streams.len();
                    for source in &resolution.streams {
                        match self.probe(source, segments, cancel).await {
                            Ok(network) => report.network.push(network),
                            Err(error) => report.error = Some(error.to_string()),
                        }
                    }
                }
            }
            report.elapsed_ms = start.elapsed().as_millis() as u64;
            report
        }))
        .buffered(self.concurrency)
        .collect()
        .await;
        Ok(reports)
    }

    pub(crate) async fn response(
        &self,
        url: &Url,
        values: &HeaderMap,
        range: Option<(u64, u64)>,
        cancel: &CancellationToken,
    ) -> Result<Response> {
        ensure!(!cancel.is_cancelled(), "Cancelled");
        validate_url(url)?;
        let mut current = url.clone();
        let origin = url.origin();
        for hop in 0..=4 {
            let mut request = self.client.get(current.clone());
            if current.origin() == origin {
                request = request.headers(values.clone());
            }
            if let Some((offset, length)) = range {
                let end = offset
                    .checked_add(length)
                    .and_then(|n| n.checked_sub(1))
                    .context("Invalid byte range")?;
                request = request.header("Range", format!("bytes={offset}-{end}"));
            }
            let response = tokio::select! {
                _ = cancel.cancelled() => bail!("Cancelled"),
                result = tokio::time::timeout(Duration::from_secs(45), request.send()) => result.map_err(|_| anyhow::anyhow!("Network timeout"))?.map_err(|error| anyhow::anyhow!("Network {}", if error.is_timeout() { "timeout" } else { "request failed" }))?,
            };
            if response.status().is_redirection() {
                ensure!(hop < 4, "Redirect limit exceeded");
                let location = response
                    .headers()
                    .get("location")
                    .and_then(|s| s.to_str().ok())
                    .context("Redirect missing location")?;
                let next = current
                    .join(location)
                    .map_err(|_| anyhow::anyhow!("Invalid redirect"))?;
                validate_url(&next)?;
                ensure!(
                    current.scheme() != "https" || next.scheme() == "https",
                    "HTTPS downgrade rejected"
                );
                current = next;
                continue;
            }
            let status = response.status();
            if status == StatusCode::TOO_MANY_REQUESTS {
                // Do not route around or retry rate limits.
                bail!("HTTP 429: rate limited; wait before another request");
            }
            ensure!(status.is_success(), "HTTP {}", status.as_u16());
            if let Some((offset, length)) = range {
                ensure!(
                    status == StatusCode::PARTIAL_CONTENT,
                    "Server ignored byte range"
                );
                let expected = format!("bytes {}-{}/", offset, offset + length - 1);
                ensure!(
                    response
                        .headers()
                        .get("content-range")
                        .and_then(|v| v.to_str().ok())
                        .is_some_and(|v| v.starts_with(&expected)),
                    "Invalid Content-Range"
                );
            }
            return Ok(response);
        }
        unreachable!()
    }

    pub(crate) async fn bytes(
        &self,
        url: &Url,
        values: &HeaderMap,
        limit: usize,
        cancel: &CancellationToken,
    ) -> Result<Vec<u8>> {
        let response = self.response(url, values, None, cancel).await?;
        self.collect(response, limit, cancel).await
    }

    pub(crate) async fn collect(
        &self,
        mut response: Response,
        limit: usize,
        cancel: &CancellationToken,
    ) -> Result<Vec<u8>> {
        ensure!(
            response.content_length().is_none_or(|n| n <= limit as u64),
            "Response exceeds size limit"
        );
        let mut bytes = Vec::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(45);
        loop {
            let chunk = tokio::select! {
                _ = cancel.cancelled() => bail!("Cancelled"),
                chunk = tokio::time::timeout_at(deadline, response.chunk()) => chunk.map_err(|_| anyhow::anyhow!("Response body timeout"))?.map_err(|_| anyhow::anyhow!("Response body transfer failed"))?,
            };
            let Some(chunk) = chunk else { break };
            ensure!(
                chunk.len() <= limit.saturating_sub(bytes.len()),
                "Response exceeds size limit"
            );
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }

    pub async fn playlist(
        &self,
        url: &Url,
        values: &BTreeMap<String, String>,
        cancel: &CancellationToken,
    ) -> Result<Playlist> {
        let response = self.response(url, &headers(values)?, None, cancel).await?;
        let final_url = response.url().clone();
        let body = self.collect(response, PLAYLIST_LIMIT, cancel).await?;
        Playlist::parse(
            &final_url,
            std::str::from_utf8(&body).context("Playlist is not UTF-8")?,
        )
    }

    /// Optional external subtitle service; keys are caller supplied, never bundled.
    pub async fn search_subtitles(
        &self,
        endpoint: &Url,
        media: &Media,
        key: Option<&str>,
        cancel: &CancellationToken,
    ) -> Result<Vec<Subtitle>> {
        media.validate()?;
        validate_url(endpoint)?;
        let mut url = endpoint.clone();
        url.query_pairs_mut()
            .append_pair("id", &media.tmdb.to_string())
            .append_pair("encoding", "utf-8")
            .append_pair("source", "all");
        if let Some(key) = key {
            url.query_pairs_mut().append_pair("key", key);
        }
        if let (Some(season), Some(episode)) = (media.season, media.episode) {
            url.query_pairs_mut()
                .append_pair("season", &season.to_string())
                .append_pair("episode", &episode.to_string());
        }
        let bytes = self
            .bytes(&url, &HeaderMap::new(), JSON_LIMIT, cancel)
            .await?;
        let captions: Vec<Subtitle> = serde_json::from_slice(&bytes)
            .map_err(|_| anyhow::anyhow!("Invalid subtitle search response"))?;
        ensure!(captions.len() <= 2048, "Subtitle results exceed limit");
        for subtitle in &captions {
            validate_url(&subtitle.url)?;
        }
        Ok(captions)
    }
}

impl Stream {
    pub fn validate(&self) -> Result<()> {
        validate_url(&self.url)?;
        headers(&self.headers)?;
        ensure!(self.subtitles.len() <= 2048, "Subtitle count exceeds limit");
        for subtitle in &self.subtitles {
            validate_url(&subtitle.url)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
