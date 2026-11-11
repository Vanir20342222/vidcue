//! Opt-in live checks against real services (`cargo test -p vidcue-desktop -- --ignored`).
//! They use a throwaway engine directory and never read the user's state, cookies or keyring.

use std::time::{Duration, Instant};

use serde_json::json;

use crate::player::mpv::{Load, Mpv};

fn engine() -> (
    vidcue_mobile::Engine,
    tokio::runtime::Runtime,
    std::path::PathBuf,
) {
    let dir = std::env::temp_dir().join(format!("vidcue-live-{}", std::process::id()));
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let e = {
        let _g = rt.enter();
        vidcue_mobile::Engine::new(&dir.join("files"), &dir.join("cache")).unwrap()
    };
    (e, rt, dir)
}

/// Search → resolve → embedded playback: frames reach the Rune surface.
#[test]
#[ignore = "live network: YouTube search, extraction and streaming"]
fn live_search_resolve_and_play_a_video() {
    let (e, rt, dir) = engine();
    let page = rt
        .block_on(e.request(json!({"op":"browse","tab":1,"request":{"Search":"big buck bunny"}})))
        .expect("search");
    let items = page["items"].as_array().cloned().unwrap_or_default();
    let media = items
        .iter()
        .find(|m| m["kind"] == "video" && m["live"] != true)
        .expect("a video result")
        .clone();
    eprintln!("playing {}", media["title"]);
    let r = rt
        .block_on(e.request(json!({"op":"resolve","media":media,"quality":"720"})))
        .expect("resolve");
    let headers: Vec<(String, String)> = r["headers"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
        .collect();
    let mpv = Mpv::new(rune::NativeSurface::new()).expect("libmpv");
    mpv.set("ao", "null");
    mpv.set_target(1280.0, 720.0);
    mpv.load(&Load {
        url: r["url"].as_str().unwrap().into(),
        audio_url: r["audio_url"].as_str().map(str::to_owned),
        headers,
        ..Default::default()
    });
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(45) && mpv.frames() < 60 {
        std::thread::sleep(Duration::from_millis(200));
        if let Some(err) = mpv.snapshot().error {
            panic!("playback failed: {err}");
        }
    }
    let snap = mpv.snapshot();
    eprintln!(
        "frames {} size {:?} duration {:?} time {:.1} hwdec {:?}",
        mpv.frames(),
        snap.video_size,
        snap.duration,
        snap.time,
        snap.hwdec
    );
    assert!(mpv.frames() >= 60, "frames: {}", mpv.frames());
    drop(mpv);
    let _ = std::fs::remove_dir_all(dir);
}

/// A radio station resolves and plays audio-only.
#[test]
#[ignore = "live network: radio-browser and a station stream"]
fn live_radio_station_plays() {
    let (e, rt, dir) = engine();
    let page = rt
        .block_on(e.request(json!({"op":"browse","tab":3,"request":{"RadioSearch":"jazz"}})))
        .expect("radio search");
    let media = page["items"]
        .as_array()
        .and_then(|a| a.first())
        .cloned()
        .expect("a station");
    let r = rt
        .block_on(e.request(json!({"op":"resolve","media":media,"audio":true})))
        .expect("resolve");
    let mpv = Mpv::new(rune::NativeSurface::new()).expect("libmpv");
    mpv.set("ao", "null");
    mpv.load(&Load {
        url: r["url"].as_str().unwrap().into(),
        audio_only: true,
        ..Default::default()
    });
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(30) && mpv.snapshot().time < 3.0 {
        std::thread::sleep(Duration::from_millis(200));
    }
    let snap = mpv.snapshot();
    eprintln!(
        "station {} time {:.1} title {:?}",
        media["title"], snap.time, snap.title
    );
    assert!(snap.time >= 3.0 && snap.error.is_none(), "{snap:?}");
    let _ = std::fs::remove_dir_all(dir);
}

/// Keyless movie metadata: Wikidata maps TMDB ids to Wikipedia posters, and search finds ids.
#[test]
#[ignore = "live network: Wikidata and Wikipedia"]
fn live_wikimedia_posters_and_search() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let client = vidcue_providers::http_client(None).unwrap();
    let ids = crate::movies::tmdb::curated()
        .iter()
        .map(|t| (t.tmdb, t.tv))
        .collect::<Vec<_>>();
    let meta = rt
        .block_on(crate::movies::wiki::enrich(&client, &ids))
        .expect("enrich");
    let posters = meta
        .values()
        .filter(|m| m.poster.starts_with("https://"))
        .count();
    eprintln!(
        "curated {} · matched {} · posters {}",
        ids.len(),
        meta.len(),
        posters
    );
    let fc = &meta[&(550, false)];
    eprintln!("550 → {:?} {} {}", fc.title, fc.year, fc.poster);
    assert!(
        posters * 10 >= ids.len() * 8,
        "most curated titles get posters"
    );
    let found = rt
        .block_on(crate::movies::wiki::search(&client, "inception"))
        .expect("search");
    eprintln!(
        "search: {:?}",
        found
            .iter()
            .map(|t| (t.tmdb, t.title.as_str(), t.year.as_str()))
            .take(5)
            .collect::<Vec<_>>()
    );
    assert!(
        found.iter().any(|t| t.tmdb == 27205 && !t.tv),
        "Inception has TMDB id 27205"
    );
}
