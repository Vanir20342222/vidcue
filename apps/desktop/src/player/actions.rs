//! What opening an item does, by kind and by where it was picked.

use vidcue_core::{BrowseRequest, Media, MediaKind};

use super::session::{Context, PlayOptions, play};
use crate::app::{App, Route, Tab};

/// A card or row was pressed. Collections open; playable items play with the rest of their
/// list as context when the list is a playlist, album or station page, and with related
/// videos otherwise.
pub fn open_item(app: &mut App, tab: Tab, media: &Media, siblings: &[Media]) {
    crate::player::preview::stop(app);
    if media.collection() {
        let tab = if tab == Tab::Radio { Tab::Home } else { tab };
        app.open_collection(tab, media);
        return;
    }
    let in_collection = matches!(&app.route, Route::Feed(f) if matches!(f.request, BrowseRequest::Collection(_) | BrowseRequest::CliampTracks(_)));
    let after: Vec<Media> = siblings
        .iter()
        .skip_while(|m| m.url != media.url)
        .skip(1)
        .filter(|m| !m.collection())
        .cloned()
        .collect();
    let context = if media.kind == MediaKind::Station || tab == Tab::Radio {
        Context::Radio(
            siblings
                .iter()
                .filter(|m| !m.collection())
                .cloned()
                .collect(),
        )
    } else if matches!(&app.route, Route::OfflinePlaylist(_)) {
        Context::Offline(siblings.to_vec())
    } else if in_collection || media.kind == MediaKind::Song {
        if after.is_empty() {
            Context::Auto
        } else {
            Context::List(
                siblings
                    .iter()
                    .filter(|m| !m.collection())
                    .cloned()
                    .collect(),
            )
        }
    } else {
        Context::Auto
    };
    let background = media.kind == MediaKind::Station;
    play(
        app,
        media.clone(),
        PlayOptions {
            context,
            background,
            ..Default::default()
        },
    );
}

/// Plays a whole collection from its first item, the engine paging the rest.
pub fn play_all(app: &mut App, tab: Tab, request: BrowseRequest, items: &[Media], shuffle: bool) {
    let playable: Vec<Media> = items.iter().filter(|m| !m.collection()).cloned().collect();
    let Some(first) = (if shuffle {
        playable.get(fastrand_index(playable.len()))
    } else {
        playable.first()
    })
    .cloned() else {
        return;
    };
    app.player.shuffle = shuffle;
    let context = match request {
        BrowseRequest::Collection(_) => Context::Collection(request, tab == Tab::Music),
        _ => Context::List(playable),
    };
    play(
        app,
        first,
        PlayOptions {
            context,
            ..Default::default()
        },
    );
}

fn fastrand_index(n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    nanos as usize % n
}
