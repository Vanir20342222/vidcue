//! Artwork: thumbnails, channel avatars and posters, fetched through the shared disk cache
//! (`vidcue_providers::images`) and handed to Rune's image cache, which builds mips off the
//! UI thread. The view asks with [`Images::get`]; misses are queued and fetched on the next
//! poll. Decoded images are bounded by count and by decoded bytes.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use rune::{Image, Waker};
use tokio::sync::Semaphore;

use crate::app::App;
use crate::backend::Backend;

const MAX_ENTRIES: usize = 420;
const MAX_BYTES: usize = 192 * 1024 * 1024;
const MAX_INFLIGHT: usize = 8;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct Size(pub u32, pub u32);

impl Size {
    pub const CARD: Size = Size(480, 270);
    pub const SQUARE: Size = Size(360, 360);
    pub const AVATAR: Size = Size(96, 96);
    pub const HERO: Size = Size(1280, 720);
    pub const POSTER: Size = Size(342, 513);
}

struct Entry {
    image: Image,
    bytes: usize,
}

pub struct Images {
    rune: Option<rune::ImageCache>,
    fetch: Option<Arc<vidcue_providers::images::ImageCache>>,
    proxy: Option<String>,
    entries: HashMap<(String, Size), Entry>,
    order: VecDeque<(String, Size)>,
    bytes: usize,
    wanted: RefCell<Vec<(String, Size)>>,
    inflight: HashSet<(String, Size)>,
    failed: HashSet<String>,
    permits: Arc<Semaphore>,
    waker: Option<Waker>,
    /// Synthetic pictures for headless scenes, by URL.
    pub fixtures: HashMap<String, Image>,
}

impl Images {
    pub fn new(be: &Backend) -> Self {
        let mut me = Self {
            rune: None,
            fetch: None,
            proxy: None,
            entries: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            wanted: RefCell::default(),
            inflight: HashSet::new(),
            failed: HashSet::new(),
            permits: Arc::new(Semaphore::new(MAX_INFLIGHT)),
            waker: None,
            fixtures: HashMap::new(),
        };
        me.configure(be, None);
        me
    }

    pub fn set_waker(&mut self, waker: Waker) {
        if self.rune.is_none() {
            self.rune = Some(rune::ImageCache::new(waker.clone()));
        }
        self.waker = Some(waker);
    }

    pub fn configure(&mut self, be: &Backend, proxy: Option<&str>) {
        if self.fetch.is_some() && self.proxy.as_deref() == proxy {
            return;
        }
        self.proxy = proxy.map(str::to_owned);
        if let Ok(client) = vidcue_providers::http_client(proxy) {
            self.fetch = Some(Arc::new(vidcue_providers::images::ImageCache::new(
                client,
                be.paths.cache.join("images"),
            )));
        }
    }

    /// The picture at `url`, if it is decoded; otherwise asks for it and returns `None`.
    pub fn get(&self, url: &str, size: Size) -> Option<Image> {
        if url.is_empty() {
            return None;
        }
        if let Some(i) = self.fixtures.get(url) {
            return Some(i.clone());
        }
        let key = (url.to_owned(), size);
        if let Some(e) = self.entries.get(&key) {
            return Some(e.image.clone());
        }
        if !self.inflight.contains(&key) && !self.failed.contains(url) {
            let mut w = self.wanted.borrow_mut();
            if !w.contains(&key) {
                w.push(key);
                if w.len() == 1
                    && let Some(waker) = &self.waker
                {
                    waker();
                }
            }
        }
        None
    }

    /// Starts fetches the last view asked for. Returns false: answers wake the app themselves.
    pub fn pump(&mut self, be: &Backend) -> bool {
        let wanted = std::mem::take(&mut *self.wanted.borrow_mut());
        if be.offline {
            return false;
        }
        let Some(fetch) = self.fetch.clone() else {
            return false;
        };
        for key in wanted {
            if self.inflight.contains(&key) || self.entries.contains_key(&key) {
                continue;
            }
            self.inflight.insert(key.clone());
            let fetch = fetch.clone();
            let permits = self.permits.clone();
            let (url, Size(w, h)) = key.clone();
            be.task(
                async move {
                    let _permit = permits.acquire_owned().await.ok();
                    let img = fetch.load_sized(&url, w, h).await?;
                    let rgba = img.to_rgba8();
                    Ok::<_, anyhow::Error>((rgba.width(), rgba.height(), rgba.into_raw()))
                },
                move |app: &mut App, out, _| app.images.landed(key, out),
            );
        }
        false
    }

    fn landed(&mut self, key: (String, Size), out: anyhow::Result<(u32, u32, Vec<u8>)>) {
        self.inflight.remove(&key);
        let Some(cache) = &self.rune else { return };
        match out {
            Ok((w, h, rgba)) if w > 0 && h > 0 => {
                let bytes = rgba.len() * 4 / 3;
                let image = cache.rgba(w, h, rgba);
                self.bytes += bytes;
                self.entries.insert(key.clone(), Entry { image, bytes });
                self.order.push_back(key);
                while self.entries.len() > MAX_ENTRIES || self.bytes > MAX_BYTES {
                    let Some(old) = self.order.pop_front() else {
                        break;
                    };
                    if let Some(e) = self.entries.remove(&old) {
                        self.bytes -= e.bytes;
                    }
                }
            }
            _ => {
                self.failed.insert(key.0);
            }
        }
    }

    /// A synthetic RGBA picture registered under a URL, for headless scenes.
    pub fn fixture(&mut self, url: &str, w: u32, h: u32, rgba: Vec<u8>) {
        if self.rune.is_none() {
            self.rune = Some(rune::ImageCache::new(std::sync::Arc::new(|| {})));
        }
        if let Some(cache) = &self.rune {
            let image = cache.rgba(w, h, rgba);
            let _ = image.wait();
            self.fixtures.insert(url.to_owned(), image);
        }
    }
}

/// The best thumbnail URL for a card of `size`.
pub fn thumb_url(media: &vidcue_core::Media, size: Size) -> String {
    vidcue_providers::images::thumbnail_url(media, size.0, size.1)
}
