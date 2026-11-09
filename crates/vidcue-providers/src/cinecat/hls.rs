use super::{Result, Url, validate_url};
use anyhow::{Context, bail, ensure};
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct Variant {
    pub url: Url,
    pub bandwidth: u64,
    pub height: Option<u32>,
    pub audio_group: Option<String>,
    pub subtitle_group: Option<String>,
}

#[derive(Clone)]
pub struct Rendition {
    pub kind: String,
    pub group: String,
    pub language: String,
    pub name: String,
    pub default: bool,
    pub url: Option<Url>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Resource {
    pub url: Url,
    pub range: Option<(u64, u64)>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Key {
    pub resource: Resource,
    pub iv: Option<String>,
}

#[derive(Clone)]
pub struct Segment {
    pub resource: Resource,
    pub duration: f64,
    pub key: Option<Key>,
    pub init: Option<Resource>,
    pub discontinuity: bool,
}

#[derive(Clone)]
pub struct Playlist {
    pub variants: Vec<Variant>,
    pub renditions: Vec<Rendition>,
    pub segments: Vec<Segment>,
    pub sequence: u64,
    pub ended: bool,
}

pub(crate) fn attributes(text: &str) -> Result<BTreeMap<String, String>> {
    let mut values = BTreeMap::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let (key, value) = rest.split_once('=').context("Malformed HLS attribute")?;
        ensure!(
            !key.is_empty() && !values.contains_key(key),
            "Invalid HLS attribute"
        );
        let (value, tail) = if let Some(quoted) = value.strip_prefix('"') {
            let end = quoted.find('"').context("Unterminated HLS attribute")?;
            let tail = &quoted[end + 1..];
            ensure!(
                tail.is_empty() || tail.starts_with(','),
                "Malformed quoted HLS attribute"
            );
            (&quoted[..end], tail.strip_prefix(',').unwrap_or(tail))
        } else {
            value.split_once(',').unwrap_or((value, ""))
        };
        values.insert(key.into(), value.into());
        rest = tail;
    }
    Ok(values)
}

fn absolute(base: &Url, value: &str) -> Result<Url> {
    let url = base
        .join(value)
        .map_err(|_| anyhow::anyhow!("Invalid HLS resource URL"))?;
    validate_url(&url)?;
    Ok(url)
}

fn byte_range(value: &str, previous_end: Option<u64>) -> Result<(u64, u64)> {
    let (length, offset) = value
        .split_once('@')
        .map_or((value, None), |(a, b)| (a, Some(b)));
    let length: u64 = length.parse().context("Invalid HLS range length")?;
    ensure!(length > 0, "Empty HLS range");
    let offset = match offset {
        Some(offset) => offset.parse().context("Invalid HLS range offset")?,
        None => previous_end.context("HLS implicit range has no previous resource")?,
    };
    offset.checked_add(length).context("HLS range overflow")?;
    Ok((offset, length))
}

impl Playlist {
    pub fn parse(base: &Url, body: &str) -> Result<Self> {
        let mut lines = body.trim_start_matches('\u{feff}').lines().map(str::trim);
        ensure!(
            lines.next() == Some("#EXTM3U"),
            "Response is not an HLS playlist"
        );
        let mut playlist = Self {
            variants: vec![],
            renditions: vec![],
            segments: vec![],
            sequence: 0,
            ended: false,
        };
        let mut variant: Option<BTreeMap<String, String>> = None;
        let mut duration = None;
        let mut range: Option<String> = None;
        let mut previous: Option<Resource> = None;
        let mut key = None;
        let mut init = None;
        let mut discontinuity = false;
        for line in lines {
            if line.is_empty() {
                continue;
            }
            if let Some(value) = line.strip_prefix("#EXT-X-STREAM-INF:") {
                ensure!(variant.is_none(), "HLS variant URI missing");
                variant = Some(attributes(value)?);
            } else if let Some(value) = line.strip_prefix("#EXT-X-MEDIA:") {
                let a = attributes(value)?;
                let kind = a.get("TYPE").context("Rendition has no type")?.clone();
                if !matches!(kind.as_str(), "AUDIO" | "SUBTITLES") {
                    continue;
                }
                playlist.renditions.push(Rendition {
                    kind,
                    group: a.get("GROUP-ID").context("Rendition has no group")?.clone(),
                    language: a.get("LANGUAGE").cloned().unwrap_or_else(|| "und".into()),
                    name: a.get("NAME").cloned().unwrap_or_default(),
                    default: a.get("DEFAULT").is_some_and(|v| v == "YES"),
                    url: a.get("URI").map(|url| absolute(base, url)).transpose()?,
                });
            } else if let Some(value) = line.strip_prefix("#EXTINF:") {
                ensure!(duration.is_none(), "HLS segment URI missing");
                let seconds: f64 = value
                    .split(',')
                    .next()
                    .unwrap_or("")
                    .parse()
                    .context("Invalid HLS duration")?;
                ensure!(seconds.is_finite() && seconds > 0.0, "Invalid HLS duration");
                duration = Some(seconds);
            } else if let Some(value) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
                ensure!(playlist.segments.is_empty(), "Late HLS media sequence");
                playlist.sequence = value.parse().context("Invalid HLS sequence")?;
            } else if let Some(value) = line.strip_prefix("#EXT-X-BYTERANGE:") {
                range = Some(value.into());
            } else if let Some(value) = line.strip_prefix("#EXT-X-KEY:") {
                let a = attributes(value)?;
                match a.get("METHOD").map(String::as_str) {
                    Some("NONE") => key = None,
                    Some("AES-128") => {
                        ensure!(
                            a.get("KEYFORMAT").is_none_or(|v| v == "identity"),
                            "Unsupported HLS key format/DRM"
                        );
                        let iv = a.get("IV").cloned();
                        if let Some(iv) = &iv {
                            ensure!(
                                iv.starts_with("0x")
                                    && iv.len() == 34
                                    && iv[2..].bytes().all(|c| c.is_ascii_hexdigit()),
                                "Invalid AES-128 IV"
                            );
                        }
                        key = Some(Key {
                            resource: Resource {
                                url: absolute(base, a.get("URI").context("HLS key has no URI")?)?,
                                range: None,
                            },
                            iv,
                        });
                    }
                    _ => bail!("Unsupported HLS encryption/DRM"),
                }
            } else if let Some(value) = line.strip_prefix("#EXT-X-MAP:") {
                let a = attributes(value)?;
                let url = absolute(base, a.get("URI").context("HLS init has no URI")?)?;
                let previous_end = init
                    .as_ref()
                    .filter(|r: &&Resource| r.url == url)
                    .and_then(|r| r.range)
                    .map(|(o, n)| o + n);
                init = Some(Resource {
                    url,
                    range: a
                        .get("BYTERANGE")
                        .map(|v| byte_range(v, previous_end))
                        .transpose()?,
                });
                ensure!(
                    key.as_ref().is_none_or(|k| k.iv.is_some()),
                    "Encrypted HLS init requires explicit IV"
                );
            } else if line == "#EXT-X-DISCONTINUITY" {
                discontinuity = true;
            } else if line == "#EXT-X-ENDLIST" {
                playlist.ended = true;
            } else if line.starts_with("#EXT-X-GAP")
                || line.starts_with("#EXT-X-DEFINE:")
                || line.starts_with("#EXT-X-SESSION-KEY:")
            {
                bail!("Unsupported HLS gap, variable or session key");
            } else if !line.starts_with('#') {
                let url = absolute(base, line)?;
                if let Some(a) = variant.take() {
                    playlist.variants.push(Variant {
                        url,
                        bandwidth: a
                            .get("AVERAGE-BANDWIDTH")
                            .or_else(|| a.get("BANDWIDTH"))
                            .context("HLS variant has no bandwidth")?
                            .parse()
                            .context("Invalid HLS bandwidth")?,
                        height: a
                            .get("RESOLUTION")
                            .and_then(|s| s.split_once('x'))
                            .map(|(_, h)| h.parse())
                            .transpose()
                            .context("Invalid HLS resolution")?,
                        audio_group: a.get("AUDIO").cloned(),
                        subtitle_group: a.get("SUBTITLES").cloned(),
                    });
                } else {
                    let previous_end = previous
                        .as_ref()
                        .filter(|r| r.url == url)
                        .and_then(|r| r.range)
                        .map(|(o, n)| o + n);
                    let resource = Resource {
                        url,
                        range: range
                            .take()
                            .map(|s| byte_range(&s, previous_end))
                            .transpose()?,
                    };
                    playlist.segments.push(Segment {
                        resource: resource.clone(),
                        duration: duration.take().context("HLS media URI has no duration")?,
                        key: key.clone(),
                        init: init.clone(),
                        discontinuity,
                    });
                    previous = Some(resource);
                    discontinuity = false;
                }
            }
            ensure!(
                playlist.segments.len() <= 100_000
                    && playlist.variants.len() <= 128
                    && playlist.renditions.len() <= 256,
                "HLS entry count exceeds limit"
            );
        }
        ensure!(
            variant.is_none() && duration.is_none(),
            "Truncated HLS playlist"
        );
        ensure!(
            !playlist.variants.is_empty() || !playlist.segments.is_empty(),
            "Empty HLS playlist"
        );
        ensure!(
            playlist.variants.is_empty() || playlist.segments.is_empty(),
            "Mixed HLS master/media playlist"
        );
        Ok(playlist)
    }

    pub fn variant(&self, max_height: Option<u32>) -> Result<&Variant> {
        self.variants
            .iter()
            .filter(|v| max_height.is_none_or(|max| v.height.is_none_or(|h| h <= max)))
            .max_by_key(|v| v.bandwidth)
            .context("No HLS variant fits the requested resolution")
    }
}
