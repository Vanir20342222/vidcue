//! What a pasted or typed link means: Android's `SharedLinks.classify`, on the shared
//! `vidcue_core` URL rules. Watch links play as one video even with a `list=` parameter.

use vidcue_core::{Media, MediaKind};

pub enum Linked {
    Video(Box<Media>),
    Playlist(String, String),
}

pub fn classify(raw: &str) -> Option<Linked> {
    let text = raw.trim();
    if text.contains(char::is_whitespace) {
        return None;
    }
    let candidate = if text.starts_with("http://") || text.starts_with("https://") {
        text.to_owned()
    } else if text.starts_with("youtu.be/")
        || text.starts_with("www.youtube.com/")
        || text.starts_with("youtube.com/")
        || text.starts_with("music.youtube.com/")
    {
        format!("https://{text}")
    } else {
        return None;
    };
    let url = vidcue_core::http_url(&candidate).ok()?;
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if !vidcue_core::youtube_host(&host) {
        return None;
    }
    let path = url.path();
    let video = url
        .query_pairs()
        .find(|(k, _)| k == "v")
        .map(|(_, v)| v.into_owned())
        .or_else(|| {
            if host.ends_with("youtu.be") {
                Some(path.trim_matches('/').to_owned())
            } else {
                ["/shorts/", "/live/", "/embed/"]
                    .iter()
                    .find_map(|p| path.strip_prefix(p).map(|v| v.trim_matches('/').to_owned()))
            }
        });
    if let Some(id) = video.filter(|v| v.len() == 11) {
        let music = host.starts_with("music.");
        return Some(Linked::Video(Box::new(Media {
            id: id.clone(),
            url: format!(
                "https://{}/watch?v={id}",
                if music {
                    "music.youtube.com"
                } else {
                    "www.youtube.com"
                }
            ),
            title: "Linked video".into(),
            kind: if music {
                MediaKind::Song
            } else {
                MediaKind::Video
            },
            ..Default::default()
        })));
    }
    if vidcue_core::is_playlist_url(&candidate)
        && let Some(list) = url
            .query_pairs()
            .find(|(k, _)| k == "list")
            .map(|(_, v)| v.into_owned())
    {
        let id = if list.starts_with("VL") {
            list
        } else {
            format!("VL{list}")
        };
        return Some(Linked::Playlist(id, "Linked playlist".into()));
    }
    if let Some(handle) = path.strip_prefix('/').filter(|p| p.starts_with('@')) {
        return Some(Linked::Playlist(
            handle.split('/').next().unwrap_or(handle).to_owned(),
            handle.to_owned(),
        ));
    }
    if let Some(channel) = path.strip_prefix("/channel/") {
        return Some(Linked::Playlist(
            channel.trim_matches('/').to_owned(),
            "Channel".into(),
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(raw: &str) -> Option<String> {
        match classify(raw)? {
            Linked::Video(m) => Some(m.id),
            Linked::Playlist(..) => None,
        }
    }

    #[test]
    fn watch_short_and_shorts_links_play_one_video() {
        assert_eq!(
            video("https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PL123").as_deref(),
            Some("dQw4w9WgXcQ")
        );
        assert_eq!(
            video("youtu.be/dQw4w9WgXcQ").as_deref(),
            Some("dQw4w9WgXcQ")
        );
        assert_eq!(
            video("https://www.youtube.com/shorts/dQw4w9WgXcQ").as_deref(),
            Some("dQw4w9WgXcQ")
        );
        assert!(
            matches!(classify("https://music.youtube.com/watch?v=dQw4w9WgXcQ"), Some(Linked::Video(m)) if m.kind == MediaKind::Song)
        );
    }

    #[test]
    fn playlists_channels_and_words_are_told_apart() {
        assert!(
            matches!(classify("https://www.youtube.com/playlist?list=PLabc"), Some(Linked::Playlist(id, _)) if id == "VLPLabc")
        );
        assert!(
            matches!(classify("https://www.youtube.com/@creator/videos"), Some(Linked::Playlist(id, _)) if id == "@creator")
        );
        assert!(classify("lofi hip hop").is_none());
        assert!(classify("https://example.com/watch?v=dQw4w9WgXcQ").is_none());
        assert!(classify("https://user:pw@www.youtube.com/watch?v=dQw4w9WgXcQ").is_none());
    }
}
