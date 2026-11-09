use super::{
    Backend, CancellationToken, JSON_LIMIT, Media, Resolution, Result, Stream, StreamKind,
    Subtitle, Url, validate_url,
};
use anyhow::{Context, ensure};
use std::collections::BTreeMap;

/// Public HTTP contracts observed in Cinecat's own browser playback.
/// Cinecat rotates between several independent ZCDN worker origins
/// (zcdn1/zcdn2/zcdn3 in its own config); each one is a separate failover
/// provider, with `zcdn` remaining the alias for the primary origin.
#[derive(Clone)]
pub struct WebSources {
    pub(crate) zcdn: Url,
    pub(crate) zcdn_backups: Vec<(String, Url)>,
    pub(crate) aphrodite: Url,
}

impl WebSources {
    pub fn new(zcdn: Url, aphrodite: Url) -> Result<Self> {
        for url in [&zcdn, &aphrodite] {
            validate_url(url)?;
            ensure!(
                url.query().is_none(),
                "Web provider base cannot have a query"
            );
        }
        Ok(Self {
            zcdn,
            zcdn_backups: vec![],
            aphrodite,
        })
    }

    /// Adds an additional independent ZCDN worker origin under its site name
    /// (for example `zcdn1`). Names and origins must be unique.
    pub fn with_zcdn_backup(mut self, name: impl Into<String>, origin: Url) -> Result<Self> {
        let name = name.into();
        ensure!(super::valid_name(&name), "Invalid provider identifier");
        validate_url(&origin)?;
        ensure!(
            origin.query().is_none(),
            "Web provider base cannot have a query"
        );
        ensure!(
            name != "zcdn" && name != "aphrodite",
            "Backup ZCDN name collides with the primary providers"
        );
        ensure!(
            self.zcdn_backups.iter().all(|(n, _)| *n != name),
            "Duplicate web provider identifier"
        );
        ensure!(
            origin != self.zcdn,
            "Backup ZCDN origin duplicates the primary origin"
        );
        self.zcdn_backups.push((name, origin));
        Ok(self)
    }

    /// Failover order of the web providers: primary ZCDN, its backup origins,
    /// then Aphrodite.
    pub(crate) fn provider_names(&self) -> Vec<String> {
        std::iter::once("zcdn".to_string())
            .chain(self.zcdn_backups.iter().map(|(n, _)| n.clone()))
            .chain(std::iter::once("aphrodite".to_string()))
            .collect()
    }
}

impl Backend {
    /// Cinecat's public Granite subtitle catalogue, with no account/key requirement.
    pub async fn search_granite_subtitles(
        &self,
        base: &Url,
        media: &Media,
        cancel: &CancellationToken,
    ) -> Result<Vec<Subtitle>> {
        media.validate()?;
        validate_url(base)?;
        let path = match (media.season, media.episode) {
            (Some(season), Some(episode)) => format!("/v1/tv/{}/{season}/{episode}", media.tmdb),
            _ => format!("/v1/movie/{}", media.tmdb),
        };
        let bytes = self
            .bytes(
                &base.join(&path)?,
                &reqwest::header::HeaderMap::new(),
                JSON_LIMIT,
                cancel,
            )
            .await?;
        #[derive(serde::Deserialize)]
        struct Entry {
            label: String,
            file: Url,
        }
        let entries: Vec<Entry> = serde_json::from_slice(&bytes)
            .map_err(|_| anyhow::anyhow!("Invalid Granite subtitle response"))?;
        ensure!(entries.len() <= 2048, "Subtitle results exceed limit");
        entries
            .into_iter()
            .map(|entry| {
                validate_url(&entry.file)?;
                let language = language(&entry.label);
                Ok(Subtitle {
                    url: entry.file,
                    language,
                    display: entry.label,
                    format: "vtt".into(),
                })
            })
            .collect()
    }

    pub(super) async fn resolve_web(
        &self,
        provider: &str,
        media: &Media,
        cancel: &CancellationToken,
    ) -> Result<Resolution> {
        let sources = self
            .web
            .as_ref()
            .context("Web providers are not configured")?;
        let started = std::time::Instant::now();
        let origin = if provider == "zcdn" {
            &sources.zcdn
        } else if provider == "aphrodite" {
            &sources.aphrodite
        } else {
            sources
                .zcdn_backups
                .iter()
                .find(|(name, _)| name == provider)
                .map(|(_, url)| url)
                .context("Unknown web provider")?
        };
        let url = if provider != "aphrodite" {
            let mut url = origin.join("/cdn")?;
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
            url
        } else {
            let path = match (media.season, media.episode) {
                (Some(season), Some(episode)) => {
                    format!("/content/tv/{}/{season}/{episode}", media.tmdb)
                }
                _ => format!("/content/movie/{}", media.tmdb),
            };
            sources.aphrodite.join(&path)?
        };
        let bytes = self
            .bytes(&url, &reqwest::header::HeaderMap::new(), JSON_LIMIT, cancel)
            .await?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|_| anyhow::anyhow!("Invalid web provider response"))?;
        if provider != "aphrodite" {
            ensure!(
                value["ready"] == true,
                "ZCDN content is not ready (queued); retry later"
            );
        } else {
            ensure!(value["found"] == true, "Aphrodite has no matching content");
        }
        let raw = value["url"]
            .as_str()
            .context("Web provider returned no stream URL")?;
        let stream_url = url
            .join(raw)
            .map_err(|_| anyhow::anyhow!("Invalid web stream URL"))?;
        let kind = if provider == "zcdn" {
            StreamKind::Hls
        } else {
            serde_json::from_value(value.get("type").cloned().unwrap_or_else(|| "hls".into()))
                .map_err(|_| anyhow::anyhow!("Unsupported web stream kind"))?
        };
        let source = Stream {
            kind,
            url: stream_url,
            // These are public playback identity headers, not an account/session.
            // Aphrodite's CDN returns a non-media redirect without the site identity.
            headers: BTreeMap::from([
                ("Origin".into(), "https://cinecat.eu".into()),
                ("Referer".into(), "https://cinecat.eu/".into()),
            ]),
            subtitles: vec![],
        };
        source.validate()?;
        Ok(Resolution {
            provider: provider.into(),
            streams: vec![source],
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    }
}

fn language(label: &str) -> String {
    let name = label
        .trim_end_matches(|c: char| c.is_ascii_digit() || c.is_whitespace())
        .trim_end_matches(" Hi")
        .trim()
        .to_ascii_lowercase();
    match name.as_str() {
        "english" => "en",
        "german" => "de",
        "french" => "fr",
        "spanish" => "es",
        "italian" => "it",
        "portuguese" => "pt",
        "portuguese brazilian" | "brazilian" => "pt-BR",
        "arabic" => "ar",
        "bulgarian" => "bg",
        "chinese" => "zh",
        "chinese simplified" => "zh-Hans",
        "chinese traditional" => "zh-Hant",
        "japanese" => "ja",
        "korean" => "ko",
        "dutch" => "nl",
        "polish" => "pl",
        "russian" => "ru",
        "ukrainian" => "uk",
        "turkish" => "tr",
        "greek" => "el",
        "czech" => "cs",
        "danish" => "da",
        "finnish" => "fi",
        "swedish" => "sv",
        "norwegian" => "no",
        "hungarian" => "hu",
        "romanian" => "ro",
        "hindi" => "hi",
        "bengali" => "bn",
        "tamil" => "ta",
        "telugu" => "te",
        "indonesian" => "id",
        "malay" => "ms",
        "vietnamese" => "vi",
        "thai" => "th",
        "hebrew" => "he",
        "persian" | "farsi" => "fa",
        "serbian" => "sr",
        "croatian" => "hr",
        "slovenian" => "sl",
        "slovak" => "sk",
        "icelandic" => "is",
        "estonian" => "et",
        "latvian" => "lv",
        "lithuanian" => "lt",
        "albanian" => "sq",
        "bosnian" => "bs",
        "macedonian" => "mk",
        _ => return name,
    }
    .into()
}
