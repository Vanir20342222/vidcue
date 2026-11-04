use crate::browse::Browser;
use anyhow::{Result, ensure};
use tokio_util::sync::CancellationToken;
use vidcue_core::{BrowseRequest, Media, MediaKind, watch::CollectionWatch};
/// An incomplete catalog must never be mistaken for a new complete baseline.
pub async fn collect(
    browser: Browser,
    watch: CollectionWatch,
    cancel: CancellationToken,
) -> Result<Vec<Media>> {
    watch.validate()?;
    let fetch = async {
        let mut request = watch.request;
        let mut tokens = std::collections::HashSet::new();
        let mut items = Vec::new();
        let mut bytes = 0usize;
        for _ in 0..32 {
            let page = browser.fetch(watch.music, &request).await?;
            ensure!(
                page.items.len() + items.len() <= 2000,
                "Collection exceeds the 2,000-entry watch limit; no new downloads were queued"
            );
            for media in &page.items {
                bytes = bytes.saturating_add(media.retained_bytes());
            }
            ensure!(
                bytes <= 16 * 1024 * 1024,
                "Watch catalog exceeds 16 MiB; no downloads were queued"
            );
            items.extend(page.items);
            let Some(token) = page.continuation else {
                return Ok(items);
            };
            ensure!(
                tokens.insert(token.clone()),
                "Collection repeated a continuation; watch was not updated"
            );
            request = BrowseRequest::Continue(token);
        }
        anyhow::bail!("Collection exceeds the 32-page watch limit; watch was not updated")
    };
    tokio::select! {
        biased;
        _ = cancel.cancelled() => anyhow::bail!("Watch check cancelled"),
        result = tokio::time::timeout(std::time::Duration::from_secs(120), fetch) => {
            result.map_err(|_| anyhow::anyhow!("Watch catalog check timed out; baseline was not changed"))?
        }
    }
}

/// Collect a collection for downloading: nested playlists and albums (an
/// artist or channel page lists them as shelves) are expanded one level into
/// their playable entries, so every returned item resolves as a single video.
/// Artist/channel entries and deeper nesting are skipped rather than resolved,
/// and duplicate videos are kept once.
pub async fn collect_playable(
    browser: Browser,
    watch: CollectionWatch,
    cancel: CancellationToken,
) -> Result<Vec<Media>> {
    let top = collect(browser.clone(), watch.clone(), cancel.clone()).await?;
    let mut seen = std::collections::HashSet::new();
    let mut items = Vec::new();
    let mut nested = 0usize;
    for media in top {
        if !media.collection() {
            if seen.insert(media.url.clone()) {
                items.push(media);
            }
            continue;
        }
        if !matches!(media.kind, MediaKind::Playlist | MediaKind::Album) {
            continue;
        }
        let id = media.browse_id.clone().unwrap_or_else(|| media.id.clone());
        if id.is_empty() {
            continue;
        }
        nested += 1;
        ensure!(
            nested <= 50,
            "Collection contains more than 50 nested playlists; no downloads were queued"
        );
        let mut child = watch.clone();
        child.request = BrowseRequest::Collection(id);
        for entry in collect(browser.clone(), child, cancel.clone()).await? {
            if !entry.collection() && seen.insert(entry.url.clone()) {
                items.push(entry);
            }
        }
        ensure!(
            items.len() <= 2000,
            "Collection exceeds the 2,000-entry watch limit; no new downloads were queued"
        );
    }
    Ok(items)
}
