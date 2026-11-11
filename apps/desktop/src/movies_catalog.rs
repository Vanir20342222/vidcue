//! Keyless title discovery and complete episode metadata, using the shared public catalog.
use super::{Episode, Title};
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use vidcue_providers::movie_catalog::MovieCatalog;

type Episodes = HashMap<u32, Vec<Episode>>;

fn parse(meta: &Value) -> Result<(Title, Episodes)> {
    let tmdb = meta["moviedb_id"]
        .as_u64()
        .or_else(|| meta["moviedb_id"].as_str()?.parse().ok())
        .filter(|id| *id > 0)
        .context("Title has no playback provider ID")?;
    let tv = meta["type"] == "series";
    let mut episodes: Episodes = HashMap::new();
    if let Some(videos) = meta["videos"].as_array() {
        for video in videos {
            let (Some(season), Some(number)) = (
                video["season"].as_u64(),
                video["episode"]
                    .as_u64()
                    .or_else(|| video["number"].as_u64()),
            ) else {
                continue;
            };
            if number == 0 || season > u32::MAX as u64 || number > u32::MAX as u64 {
                continue;
            }
            episodes.entry(season as u32).or_default().push(Episode {
                number: number as u32,
                name: video["name"].as_str().unwrap_or_default().to_owned(),
                overview: video["overview"]
                    .as_str()
                    .or_else(|| video["description"].as_str())
                    .unwrap_or_default()
                    .to_owned(),
                runtime: None,
            });
        }
    }
    for entries in episodes.values_mut() {
        entries.sort_by_key(|e| e.number);
        entries.dedup_by_key(|e| e.number);
    }
    Ok((
        Title {
            tmdb,
            tv,
            imdb: meta["id"].as_str().context("Missing title ID")?.to_owned(),
            title: meta["name"].as_str().unwrap_or_default().to_owned(),
            overview: meta["description"].as_str().unwrap_or_default().to_owned(),
            year: meta["releaseInfo"]
                .as_str()
                .or_else(|| meta["year"].as_str())
                .unwrap_or_default()
                .chars()
                .take(4)
                .collect(),
            poster: meta["poster"].as_str().unwrap_or_default().to_owned(),
            backdrop: meta["background"].as_str().unwrap_or_default().to_owned(),
            genres: meta["genres"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
            ..Default::default()
        },
        episodes,
    ))
}

pub async fn search(query: &str, proxy: Option<&str>) -> Result<(Vec<Title>, Option<String>)> {
    let catalog = MovieCatalog::with_proxy(proxy)?;
    let result = catalog.search(query).await?;
    let items = result["titles"]
        .as_array()
        .context("Invalid catalog results")?;
    // Search summaries lack TMDB IDs. Enrich with bounded concurrency before handing off to playback.
    use futures_util::{StreamExt, stream};
    let catalog = &catalog;
    let entries = stream::iter(items.iter().cloned().map(|item| async move {
        let kind = item["type"].as_str().context("Missing media type")?;
        let id = item["id"].as_str().context("Missing title ID")?;
        let meta = catalog.details(kind, id).await?;
        parse(&meta).map(|(title, _)| title)
    }))
    .buffered(4)
    .collect::<Vec<_>>()
    .await;
    let mut titles = Vec::new();
    let mut failed = false;
    for entry in entries {
        match entry {
            Ok(title) => titles.push(title),
            Err(_) => failed = true,
        }
    }
    anyhow::ensure!(
        !titles.is_empty() || !failed,
        "Title metadata unavailable; try searching again"
    );
    let mut warnings = result["warnings"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if failed {
        warnings.push("Some title metadata could not be loaded; try again".into());
    }
    Ok((titles, (!warnings.is_empty()).then(|| warnings.join(". "))))
}

pub async fn details(tv: bool, imdb: &str, proxy: Option<&str>) -> Result<(Title, Episodes)> {
    let meta = MovieCatalog::with_proxy(proxy)?
        .details(if tv { "series" } else { "movie" }, imdb)
        .await?;
    parse(&meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn cyberpunk_metadata_includes_every_episode_and_multiple_seasons() {
        let meta = json!({"id":"tt12590266", "type":"series", "moviedb_id":105248,
            "name":"Cyberpunk: Edgerunners", "videos": (0..=2).flat_map(|season| (1..=10).map(move |episode| json!({"season":season,"episode":episode}))).collect::<Vec<_>>()});
        let (title, episodes) = parse(&meta).unwrap();
        assert_eq!(title.tmdb, 105248);
        assert!(title.tv);
        assert_eq!(episodes.len(), 3);
        assert_eq!(episodes[&1].len(), 10);
        assert_eq!(episodes[&2][9].number, 10);
    }
}
