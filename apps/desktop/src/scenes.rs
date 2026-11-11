//! Headless scenes: synthetic, offline states for `--snap` renders and visual checks.
//! Nothing here touches the network or the user's files; artwork is drawn procedurally.
//!
//! ```sh
//! vidcue-desktop --snap home.png --scene home --size 1440x900
//! ```

use vidcue_core::{BrowseRequest, Chapter, Media, MediaKind};

use crate::app::{App, FeedRoute, LibraryTab, Route, SettingsSection, Tab};
use crate::feed::FeedState;
use crate::player::session::{Now, Source};

type Scene = (&'static str, fn(&mut App));

pub const SCENES: &[Scene] = &[
    ("home", home),
    ("home-light", |a| {
        theme(a, "flexoki-light");
        home(a)
    }),
    ("home-nothing", |a| {
        theme(a, "nothing-dark");
        home(a)
    }),
    ("music", music),
    ("music-loading", |a| {
        let r = FeedRoute::root(Tab::Music);
        a.feeds.pages.insert(
            r.key(),
            FeedState {
                loading: true,
                ..Default::default()
            },
        );
        a.route = Route::Feed(r);
    }),
    ("radio", radio),
    ("collection", collection),
    ("watch", watch),
    ("watch-menu", |a| {
        watch(a);
        a.player.menu = Some(crate::player::session::PlayerMenu::Settings);
        a.player.snap.paused = true;
    }),
    ("song", |a| song(a, "classic")),
    ("song-vinyl", |a| song(a, "vinyl")),
    ("song-vu", |a| song(a, "vu")),
    ("song-orbit", |a| song(a, "orbit")),
    ("song-lyrics", |a| {
        song(a, "classic");
        a.prefs.show_lyrics = true;
    }),
    ("dock", |a| {
        song(a, "vinyl");
        a.route = Route::Feed(FeedRoute::root(Tab::Home));
        home_items(a);
    }),
    ("downloads", downloads),
    ("library-queue", |a| {
        watch(a);
        a.route = Route::Library(LibraryTab::Queue);
    }),
    ("library-saved", |a| {
        a.library.saved = videos(8, 40);
        a.saved_urls = a.library.saved.iter().map(|m| m.url.clone()).collect();
        a.route = Route::Library(LibraryTab::Saved);
    }),
    ("settings", |a| {
        a.route = Route::Settings(SettingsSection::Appearance)
    }),
    ("settings-playback", |a| {
        a.route = Route::Settings(SettingsSection::Playback)
    }),
    ("account", |a| a.route = Route::Account),
    ("movies", |a| {
        a.prefs.alpha_movies = true;
        a.prefs.alpha_notice_accepted = true;
        a.route = Route::Movies;
    }),
    ("update-offer", |a| {
        home(a);
        a.update.method = crate::updater::Method::WindowsInstaller;
        a.update.state = crate::updater::State::Available(sample_release());
        a.update.prompt = true;
    }),
    ("update-downloading", |a| {
        home(a);
        a.update.method = crate::updater::Method::WindowsInstaller;
        a.update.state = crate::updater::State::Installing(sample_release());
        a.update
            .progress
            .bytes
            .store(23_400_000, std::sync::atomic::Ordering::Relaxed);
        a.update.prompt = true;
    }),
    ("settings-update-building", |a| {
        a.update.method = crate::updater::Method::Source {
            launcher: "/home/user/.local/bin/vidcue".into(),
        };
        a.update.state = crate::updater::State::Installing(sample_release());
        *a.update.progress.line.lock().unwrap() = "Compiling vidcue-desktop v0.2.1".into();
        a.route = Route::Settings(SettingsSection::About);
    }),
    ("onboarding", |a| a.onboarding = Some(0)),
    ("onboarding-theme", |a| a.onboarding = Some(1)),
    ("toast", |a| {
        home(a);
        a.toast(
            crate::app::ToastKind::Success,
            "Saved “Northern Lights Over Tromsø”",
        );
    }),
];

fn sample_release() -> vidcue_services::update::Release {
    vidcue_services::update::Release {
        tag: "desktop-0.2.1".into(),
        version: "0.2.1".into(),
        name: "Vidcue 0.2.1".into(),
        url: "https://example.invalid/releases/desktop-0.2.1".into(),
        notes: vec![
            "Update from inside the app on Windows and Linux".into(),
            "Faster search".into(),
        ],
        assets: vec![vidcue_services::update::Asset {
            name: "VidcueSetup-0.2.1.exe".into(),
            url: "https://example.invalid/VidcueSetup-0.2.1.exe".into(),
            size: 60_611_529,
            sha256: None,
        }],
    }
}

fn theme(a: &mut App, name: &str) {
    a.config.theme = name.into();
    if let Ok(p) = vidcue_core::theme::builtin(name) {
        a.theme = crate::theme::Theme::from_palette(&p);
    }
}

const TITLES: [&str; 16] = [
    "Northern Lights Over Tromsø in 8K",
    "How GPUs Draw Text, Explained",
    "A Week Living in a Tiny Cabin",
    "The Fastest Way to Learn Rust in 2026",
    "Street Food Tour of Osaka",
    "Building a Modular Synth From Scratch",
    "Why Trains Are Coming Back",
    "Deep Sea Creatures You've Never Seen",
    "Lo-fi Beats to Code To",
    "Restoring a 1972 Porsche 911",
    "The Physics of a Perfect Espresso",
    "Climbing the Dolomites Solo",
    "Inside the World's Largest Library",
    "Mars Rover: One Year Later",
    "Minimalist Desk Setup Tour",
    "Jazz at the Blue Note, Live",
];
const CREATORS: [&str; 8] = [
    "Aurora Lab",
    "Byte Sized",
    "Field Notes",
    "Kitchen Atlas",
    "Wavelength",
    "Rail Stories",
    "Ocean Deep",
    "Studio North",
];

/// A procedural picture: sky gradient, a sun, hills, all from `seed`.
fn picture(seed: u32, w: u32, h: u32) -> Vec<u8> {
    let hue = (seed as f32 * 47.0) % 360.0;
    let c = |l: f32, ch: f32, hh: f32| {
        let col = rune::oklch(l, ch, hh);
        let enc = |v: f32| {
            (if v <= 0.0031308 {
                v * 12.92
            } else {
                1.055 * v.powf(1.0 / 2.4) - 0.055
            }
            .clamp(0.0, 1.0)
                * 255.0) as u8
        };
        [enc(col.r), enc(col.g), enc(col.b)]
    };
    let top = c(0.32, 0.12, hue);
    let bottom = c(0.72, 0.14, (hue + 40.0) % 360.0);
    let sun = c(0.93, 0.08, (hue + 70.0) % 360.0);
    let hill = c(0.22, 0.06, (hue + 200.0) % 360.0);
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    let sx = 0.25 + (seed % 5) as f32 * 0.12;
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let mut rgb = [0f32; 3];
            for i in 0..3 {
                rgb[i] = top[i] as f32 + (bottom[i] as f32 - top[i] as f32) * v;
            }
            let d = ((u - sx).powi(2) + ((v - 0.4) * h as f32 / w as f32).powi(2)).sqrt();
            let glow = (1.0 - d / 0.12).clamp(0.0, 1.0).powf(0.5);
            for i in 0..3 {
                rgb[i] += (sun[i] as f32 - rgb[i]) * glow;
            }
            let ridge = 0.68
                + 0.08 * (u * 7.0 + seed as f32).sin()
                + 0.04 * (u * 19.0 + seed as f32 * 0.3).sin();
            let land = ((v - ridge) * h as f32 + 0.5).clamp(0.0, 1.0);
            for i in 0..3 {
                rgb[i] += (hill[i] as f32 - rgb[i]) * land;
            }
            px.extend(rgb.iter().map(|v| v.clamp(0.0, 255.0) as u8));
            px.push(255);
        }
    }
    px
}

fn art(a: &mut App, seed: u32, square: bool) -> String {
    let url = format!(
        "fixture://art/{seed}/{}",
        if square { "sq" } else { "wide" }
    );
    let (w, h) = if square { (360, 360) } else { (480, 270) };
    a.images.fixture(&url, w, h, picture(seed, w, h));
    url
}

fn videos(n: usize, seed: u32) -> Vec<Media> {
    (0..n)
        .map(|i| {
            let s = seed + i as u32;
            Media {
                id: format!("fixture{s:05}"),
                title: TITLES[i % TITLES.len()].into(),
                creator: CREATORS[i % CREATORS.len()].into(),
                url: format!("https://www.youtube.com/watch?v=fixture{s:04}"),
                thumbnail: format!("fixture://art/{s}/wide"),
                duration: Some(180 + (s as u64 * 137) % 3000),
                view_count: Some(1_200 + (s as u64 * 977_331) % 9_000_000),
                published_text: format!("{} days ago", 1 + s % 28),
                channel_id: format!("UCfixture{s}"),
                live: i == 5,
                kind: MediaKind::Video,
                ..Default::default()
            }
        })
        .collect()
}

fn register(a: &mut App, items: &[Media], square: bool) {
    for m in items {
        if let Some(seed) = m.thumbnail.split('/').nth(3).and_then(|s| s.parse().ok()) {
            art(a, seed, square);
        }
    }
}

fn fill(a: &mut App, route: FeedRoute, items: Vec<Media>, title: &str) {
    a.feeds.pages.insert(
        route.key(),
        FeedState {
            title: title.into(),
            items,
            fetched_at: Some(0.0),
            continuation: Some("more".into()),
            ..Default::default()
        },
    );
    a.route = Route::Feed(route);
}

fn home_items(a: &mut App) {
    let items = videos(16, 1);
    register(a, &items, false);
    let r = FeedRoute::root(Tab::Home);
    a.feeds.pages.insert(
        r.key(),
        FeedState {
            title: "Home".into(),
            items,
            fetched_at: Some(0.0),
            continuation: Some("more".into()),
            ..Default::default()
        },
    );
}

fn home(a: &mut App) {
    let items = videos(16, 1);
    register(a, &items, false);
    for m in &items {
        let _ = m;
    }
    // The hero uses a larger picture.
    a.images
        .fixture(&items[0].thumbnail, 480, 270, picture(1, 480, 270));
    fill(a, FeedRoute::root(Tab::Home), items, "Home");
}

fn songs(n: usize, seed: u32, kind: MediaKind) -> Vec<Media> {
    let names = [
        "Midnight Drive",
        "Paper Planes",
        "Glass Garden",
        "Low Tide",
        "Neon Rain",
        "Afterglow",
        "Copper Sky",
        "Static Hearts",
        "Slow Bloom",
        "Echo Park",
    ];
    (0..n)
        .map(|i| {
            let s = seed + i as u32;
            Media {
                id: format!("song{s:05}"),
                title: names[i % names.len()].into(),
                creator: CREATORS[(i + 3) % CREATORS.len()].into(),
                url: format!("https://music.youtube.com/watch?v=song{s:05}xx"),
                thumbnail: format!("fixture://art/{s}/sq"),
                duration: Some(150 + (s as u64 * 31) % 160),
                kind,
                browse_id: (kind == MediaKind::Album).then(|| format!("MPREfixture{s}")),
                ..Default::default()
            }
        })
        .collect()
}

fn music(a: &mut App) {
    let items = songs(14, 100, MediaKind::Album);
    register(a, &items, true);
    fill(a, FeedRoute::root(Tab::Music), items, "Music");
}

fn collection(a: &mut App) {
    let items = songs(12, 200, MediaKind::Song);
    register(a, &items, true);
    let r = FeedRoute {
        tab: Tab::Music,
        request: BrowseRequest::Collection("MPREfixture".into()),
        title: "Glass Garden".into(),
    };
    fill(a, r, items, "Glass Garden");
}

fn radio(a: &mut App) {
    let names = [
        "Groove Salad",
        "Radio Paradise",
        "KEXP Seattle",
        "FIP Paris",
        "Jazz24",
        "NTS 1",
        "BBC 6 Music",
        "Lush",
    ];
    let items: Vec<Media> = (0..8)
        .map(|i| Media {
            id: format!("station{i}"),
            title: names[i].into(),
            creator: "Internet radio".into(),
            url: format!("https://radio.example/{i}.mp3"),
            thumbnail: format!("fixture://art/{}/sq", 300 + i),
            kind: MediaKind::Station,
            live: true,
            section: ["ambient", "eclectic", "indie", "jazz"][i % 4].into(),
            ..Default::default()
        })
        .collect();
    register(a, &items, true);
    fill(a, FeedRoute::root(Tab::Radio), items, "Radio");
}

fn now(a: &mut App, media: Media, audio: bool, source: Source) {
    let mut n = Now::new(media, audio, source, "1080".into(), 1);
    n.resolving = false;
    n.qualities = vec![2160, 1440, 1080, 720, 480];
    n.captions = vec![
        ("English".into(), "fixture://c/en".into()),
        ("Deutsch".into(), "fixture://c/de".into()),
    ];
    n.details = serde_json::json!({ "published_text": "3 days ago", "like_count": 48211, "subscribers_text": "1.2M subscribers" });
    a.player.now = Some(n);
    a.player.snap.loaded = true;
    a.player.snap.duration = Some(1342.0);
    a.player.snap.time = 412.0;
    a.player.snap.paused = false;
    a.player.lives.set_progress(412.0 / 1342.0);
    a.player.lives.time.set("6:52");
    a.player.lives.time_of.set("6:52 / 22:22");
}

fn watch(a: &mut App) {
    let items = videos(12, 1);
    register(a, &items, false);
    let mut m = items[0].clone();
    m.description = "Filmed over three nights above the Arctic circle. Gear list and settings in the pinned comment.\n\n0:00 Intro\n2:15 First night\n9:40 The big display\n17:05 Timelapse\n\nMusic: https://example.com/soundtrack".into();
    m.chapters = vec![
        Chapter {
            title: "Intro".into(),
            start: 0.0,
            end: Some(135.0),
        },
        Chapter {
            title: "First night".into(),
            start: 135.0,
            end: Some(580.0),
        },
        Chapter {
            title: "The big display".into(),
            start: 580.0,
            end: Some(1025.0),
        },
        Chapter {
            title: "Timelapse".into(),
            start: 1025.0,
            end: Some(1342.0),
        },
    ];
    now(a, m, false, Source::Stream);
    a.player.queue = items[1..4].to_vec();
    a.player.context = serde_json::json!({ "related": true, "items": items[4..10] });
    a.player.sponsor = vec![vidcue_providers::sponsorblock::Segment {
        start: 300.0,
        end: 360.0,
        category: "sponsor".into(),
    }];
    a.route = Route::Watch;
}

fn song(a: &mut App, style: &str) {
    let items = songs(8, 200, MediaKind::Song);
    register(a, &items, true);
    a.config.music_player_style = style.into();
    now(a, items[0].clone(), true, Source::Stream);
    a.player.snap.duration = Some(214.0);
    a.player.snap.time = 63.0;
    a.player.lives.set_progress(63.0 / 214.0);
    a.player.lives.set_levels(0.78, 0.7, 0.86);
    a.player.context = serde_json::json!({ "items": items[1..] });
    let lines = vidcue_providers::lyrics::parse(
        "[00:45.00]Headlights paint the empty road\n[00:52.00]Radio hums a song I know\n[00:59.00]Every mile a little slower\n[01:05.00]Midnight drive, I'm going home\n[01:12.00]City fading in the mirror\n[01:19.00]Stars are getting closer, clearer",
    )
    .unwrap_or_default();
    let l = &mut a.player.lyrics;
    l.url = a
        .player
        .now
        .as_ref()
        .map(|n| n.media.url.clone())
        .unwrap_or_default();
    l.tracks = vec![vidcue_providers::lyrics::Track {
        title: "Midnight Drive".into(),
        artist: "Wavelength".into(),
        album: String::new(),
        duration: 214.0,
        instrumental: false,
        lines,
    }];
    l.active = Some(3);
    a.route = Route::Watch;
}

fn downloads(a: &mut App) {
    use vidcue_core::{Job, JobState, Progress};
    let items = videos(6, 20);
    register(a, &items, false);
    let states = [
        JobState::Downloading,
        JobState::Downloading,
        JobState::Queued,
        JobState::Merging,
        JobState::Error,
        JobState::Paused,
    ];
    for (i, m) in items.into_iter().enumerate() {
        let mut j = Job::new(
            m,
            vidcue_core::DownloadOptions {
                audio: i == 1,
                ..Default::default()
            },
        );
        j.state = states[i];
        j.progress = Progress {
            downloaded: 40_000_000 + i as u64 * 9_000_000,
            total: Some(120_000_000),
            speed: Some(3_400_000.0),
            eta: Some(42.0),
        };
        if j.state == JobState::Error {
            j.error = Some("HTTP 403: the stream URL expired; retry renews it".into());
        }
        a.downloads.queue.jobs.push(j);
    }
    a.route = Route::Downloads;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pictures_are_full_rgba_and_differ_by_seed() {
        let a = picture(1, 32, 18);
        let b = picture(2, 32, 18);
        assert_eq!(a.len(), 32 * 18 * 4);
        assert_ne!(a, b);
        assert!(a.chunks(4).all(|p| p[3] == 255));
    }

    #[test]
    fn fixture_media_are_well_formed() {
        for m in videos(16, 1) {
            assert!(vidcue_core::http_url(&m.url).is_ok());
            assert!(m.thumbnail.starts_with("fixture://"));
        }
    }
}
