//! Shared building blocks. Every screen draws with these so spacing, motion and colour stay
//! consistent: buttons and chips spring on press, cards lift on hover, menus and toasts enter
//! with presence transitions, sliders follow the pointer without re-running layout.

use rune::tok::motion;
use rune::{
    Align, Color, Cycle, Drag, El, Icon, Phase, Side, TextStyle, Transition, Tx, col, el, icon,
    row, spacer, text,
};
use vidcue_core::{Media, MediaKind};

use crate::app::{App, MenuAction, MenuItem, MenuState, ToastKind};
use crate::icons as ic;
use crate::images::{Size, thumb_url};
use crate::theme::{Theme, sz, ty};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The one inverted pill per screen.
    Primary,
    Secondary,
    Ghost,
    Danger,
    Glass,
}

/// A pill button. Attach `.on_press(..)` to the result.
pub fn button(t: &Theme, label: &str, glyph: Option<&'static Icon>, kind: Kind) -> El {
    let (bg, fg, hover) = match kind {
        Kind::Primary => (t.text, t.bg, t.text.mix(t.bg, 0.12)),
        Kind::Secondary => (t.raised, t.text, t.overlay),
        Kind::Ghost => (Color::TRANSPARENT, t.text, t.hover),
        Kind::Danger => (t.error.alpha(0.16), t.error, t.error.alpha(0.24)),
        Kind::Glass => (
            Color::WHITE.alpha(0.14),
            Color::WHITE,
            Color::WHITE.alpha(0.24),
        ),
    };
    let mut b = row()
        .h(38.0)
        .px(16.0)
        .gap(8.0)
        .center_y()
        .radius(sz::R_PILL)
        .fill(bg)
        .hover(hover)
        .press_scale(0.96)
        .role("button")
        .label(label)
        .focusable(true)
        .cursor(rune::Cursor::Pointer);
    if kind == Kind::Glass {
        b = b.border(1.0, Color::WHITE.alpha(0.22));
    }
    if let Some(g) = glyph {
        b = b.child(icon(g, 17.0).color(fg));
    }
    b.child(text(label).font(ty::BUTTON).color(fg))
}

/// A round icon-only button with a tooltip.
pub fn icon_button(t: &Theme, glyph: &'static Icon, label: &str, size: f32) -> El {
    el().size(size, size)
        .radius(size / 2.0)
        .center()
        .hover(t.hover)
        .press_scale(0.9)
        .role("button")
        .label(label)
        .help(label)
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .child(icon(glyph, (size * 0.5).round()).color(t.text))
}

/// The same over video: white glyph on a translucent disc.
pub fn overlay_button(glyph: &'static Icon, label: &str, size: f32) -> El {
    el().size(size, size)
        .radius(size / 2.0)
        .center()
        .hover(Color::WHITE.alpha(0.16))
        .press_scale(0.88)
        .role("button")
        .label(label)
        .help(label)
        .cursor(rune::Cursor::Pointer)
        .child(icon(glyph, (size * 0.52).round()).color(Color::WHITE))
}

pub fn chip(t: &Theme, label: &str, selected: bool) -> El {
    let (bg, fg) = if selected {
        (t.text, t.bg)
    } else {
        (t.raised, t.text)
    };
    row()
        .h(34.0)
        .px(14.0)
        .center_y()
        .radius(sz::R_PILL)
        .fill(bg)
        .hover(if selected { bg } else { t.overlay })
        .press_scale(0.95)
        .motion(motion::SNAP)
        .role("tab")
        .checked(selected)
        .label(label)
        .cursor(rune::Cursor::Pointer)
        .child(
            text(label)
                .font(if selected {
                    ty::SMALL_STRONG
                } else {
                    ty::SMALL
                })
                .color(fg),
        )
}

pub fn badge(text_: &str, bg: Color, fg: Color) -> El {
    row().h(20.0).px(7.0).center_y().radius(5.0).fill(bg).child(
        text(text_)
            .font(TextStyle::new(11.0, 650.0).features("'tnum' 1"))
            .color(fg),
    )
}

/// A switch with a knob that springs across.
pub fn switch(t: &Theme, key: &str, on: bool) -> El {
    let knob = el()
        .key(format!("{key}-knob"))
        .size(18.0, 18.0)
        .radius(9.0)
        .fill(if on { t.on_accent } else { Color::WHITE })
        .shadow(1.0, 3.0, Color::BLACK.alpha(0.25))
        .absolute(if on { 21.0 } else { 3.0 }, 3.0);
    el().key(key.to_owned())
        .role("switch")
        .checked(on)
        .focusable(true)
        .size(42.0, 24.0)
        .radius(12.0)
        .fill(if on { t.accent } else { t.overlay })
        .motion(motion::POP)
        .cursor(rune::Cursor::Pointer)
        .child(knob)
}

/// A setting row: title, optional description, and a control on the right.
pub fn setting_row(t: &Theme, title: &str, detail: &str, control: El) -> El {
    let mut words = col()
        .grow(1.0)
        .shrink()
        .gap(3.0)
        .child(text(title).font(ty::BODY_STRONG).color(t.text));
    if !detail.is_empty() {
        words = words.child(text(detail).font(ty::SMALL).color(t.muted).wrap());
    }
    row()
        .w_full()
        .py(12.0)
        .gap(24.0)
        .center_y()
        .child(words)
        .child(control)
}

/// A horizontal slider. `on_change` gets the new value in `0..=1` while dragging.
pub fn slider(
    t: &Theme,
    key: &str,
    value: f32,
    width: f32,
    on_change: fn(&mut App, f32, bool),
) -> El {
    let v = value.clamp(0.0, 1.0);
    let track = el()
        .w(width)
        .h(4.0)
        .radius(2.0)
        .fill(t.overlay)
        .child(el().w(width * v).h(4.0).radius(2.0).fill(t.accent));
    let knob = el()
        .size(16.0, 16.0)
        .radius(8.0)
        .fill(Color::WHITE)
        .shadow(1.0, 4.0, Color::BLACK.alpha(0.35))
        .absolute(width * v - 8.0, 2.0);
    el().key(key.to_owned())
        .w(width)
        .h(20.0)
        .role("slider")
        .range_value(v as f64, 0.0, 1.0)
        .cursor(rune::Cursor::Pointer)
        .child(el().absolute(0.0, 8.0).child(track))
        .child(knob)
        .on_drag(move |s: &mut App, d: &mut Drag, tx: &mut Tx| {
            tx.instant();
            on_change(
                s,
                (d.x / d.w.max(1.0)).clamp(0.0, 1.0),
                d.phase == Phase::Up,
            );
        })
}

/// A segmented control; `on_pick` receives the index.
pub fn segmented(
    t: &Theme,
    key: &str,
    labels: &[&str],
    selected: usize,
    on_pick: fn(&mut App, usize),
) -> El {
    let mut r = row()
        .key(key.to_owned())
        .padding(3.0)
        .gap(2.0)
        .radius(sz::R_PILL)
        .fill(t.raised)
        .focus_group();
    for (i, l) in labels.iter().enumerate() {
        let on = i == selected;
        r = r.child(
            row()
                .key(format!("{key}-{i}"))
                .h(30.0)
                .px(14.0)
                .center()
                .radius(sz::R_PILL)
                .fill(if on { t.overlay } else { Color::TRANSPARENT })
                .shadow(
                    if on { 1.0 } else { 0.0 },
                    if on { 4.0 } else { 0.0 },
                    Color::BLACK.alpha(if on { 0.2 } else { 0.0 }),
                )
                .hover(if on { t.overlay } else { t.hover })
                .motion(motion::SNAP)
                .role("radio")
                .checked(on)
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| on_pick(s, i))
                .child(
                    text(*l)
                        .font(if on { ty::SMALL_STRONG } else { ty::SMALL })
                        .color(if on { t.text } else { t.muted }),
                ),
        );
    }
    r
}

pub fn section_header(t: &Theme, title: &str, subtitle: Option<&str>) -> El {
    let mut c = col().gap(2.0).child(text(title).font(ty::H2).color(t.text));
    if let Some(s) = subtitle {
        c = c.child(text(s).font(ty::SMALL).color(t.muted));
    }
    c
}

pub fn page_title(t: &Theme, title: &str, subtitle: Option<&str>) -> El {
    let mut c = col()
        .gap(4.0)
        .pb(8.0)
        .child(text(title).font(ty::H1).color(t.text));
    if let Some(s) = subtitle {
        c = c.child(text(s).font(ty::BODY).color(t.muted));
    }
    c
}

/// A card surface.
pub fn panel(t: &Theme) -> El {
    col()
        .radius(sz::R_PANEL)
        .fill(t.surface)
        .border(1.0, t.hairline)
}

pub fn empty_state(t: &Theme, glyph: &'static Icon, title: &str, body: &str) -> El {
    col()
        .w_full()
        .py(64.0)
        .gap(12.0)
        .center()
        .transition(Transition::scale(0.96))
        .child(
            el().size(72.0, 72.0)
                .radius(36.0)
                .fill(t.accent_soft)
                .center()
                .child(icon(glyph, 32.0).color(t.accent)),
        )
        .child(text(title).font(ty::H3).color(t.text))
        .child(
            text(body)
                .font(ty::BODY)
                .color(t.muted)
                .max_w(460.0)
                .wrap()
                .align(rune::Align::Center),
        )
}

pub fn error_state(t: &Theme, title: &str, body: &str) -> El {
    col()
        .w_full()
        .py(56.0)
        .gap(12.0)
        .center()
        .transition(Transition::scale(0.96))
        .child(
            el().size(64.0, 64.0)
                .radius(32.0)
                .fill(t.error.alpha(0.14))
                .center()
                .child(icon(&ic::WARNING, 28.0).color(t.error)),
        )
        .child(text(title).font(ty::H3).color(t.text))
        .child(
            text(body)
                .font(ty::BODY)
                .color(t.muted)
                .max_w(520.0)
                .wrap()
                .align(rune::Align::Center),
        )
}

/// A soft pulse for loading placeholders.
pub const PULSE: Cycle = Cycle {
    period: 1.4,
    phase: 0.0,
    stops: &[(0.0, 0.55), (0.5, 1.0), (1.0, 0.55)],
    still: 0.8,
    fps: 30.0,
};

/// A placeholder block with a soft glint gliding across it, as Android's `shimmer`.
pub fn shimmer_block(t: &Theme, w: f32, h: f32, radius: f32) -> El {
    let base = t.raised;
    let glint = base.mix(t.text, 0.09);
    let band = (w * 0.9).max(120.0);
    el().w(w)
        .h(h)
        .radius(radius)
        .clip_rounded(radius)
        .fill(base)
        .child(
            el().absolute(0.0, 0.0)
                .w(band)
                .h(h)
                .linear_gradient(
                    std::f32::consts::FRAC_PI_2,
                    [(0.0, base.alpha(0.0)), (0.5, glint), (1.0, base.alpha(0.0))],
                )
                .looping(rune::LoopSpec::slide(1.6, [-band, 0.0], [w + band, 0.0])),
        )
}

/// A loading card shaped like the cards that replace it: 16:9 with a channel avatar for
/// videos, square for music and stations.
pub fn skeleton_card(t: &Theme, width: f32, i: usize, square: bool) -> El {
    let art_h = if square { width } else { width * 9.0 / 16.0 };
    let lines = col()
        .gap(8.0)
        .grow(1.0)
        .child(shimmer_block(
            t,
            (width - if square { 0.0 } else { 46.0 }) * 0.82,
            14.0,
            6.0,
        ))
        .child(shimmer_block(
            t,
            (width - if square { 0.0 } else { 46.0 }) * 0.5,
            12.0,
            6.0,
        ));
    let mut info = row().w(width).gap(10.0).pt(2.0);
    if !square {
        info = info.child(shimmer_block(t, 36.0, 36.0, 18.0));
    }
    col()
        .key(format!("skeleton-{i}"))
        .w(width)
        .gap(12.0)
        .label("Loading…")
        .child(shimmer_block(t, width, art_h, sz::R_CARD))
        .child(info.child(lines))
}

pub fn count(n: u64) -> String {
    vidcue_core::compact_count(n)
}

pub fn duration_text(m: &Media) -> Option<String> {
    if m.live {
        return Some("LIVE".into());
    }
    m.duration.map(|d| crate::player::mpv::clock(d as f64))
}

/// "Channel · 1.2M views · 3 years ago", by the user's card settings.
pub fn meta_line(app: &App, m: &Media) -> String {
    let info = &app.config.card_info;
    let mut parts: Vec<String> = Vec::new();
    if info.channel && !m.creator.is_empty() {
        parts.push(m.creator.clone());
    }
    if info.views {
        if !m.views_text.is_empty() {
            parts.push(m.views_text.clone());
        } else if let Some(v) = m.view_count {
            parts.push(format!("{} views", count(v)));
        }
    }
    if info.upload_date {
        if !m.published_text.is_empty() {
            parts.push(m.published_text.clone());
        } else if let Some(d) = &m.upload_date
            && d.len() == 8
        {
            parts.push(format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..]));
        }
    }
    if info.section && !m.section.is_empty() {
        parts.push(m.section.clone());
    }
    parts.join(" · ")
}

fn kind_label(k: MediaKind) -> &'static str {
    match k {
        MediaKind::Playlist => "Playlist",
        MediaKind::Album => "Album",
        MediaKind::Artist => "Artist",
        MediaKind::Channel => "Channel",
        MediaKind::Station => "Station",
        MediaKind::Song => "Song",
        MediaKind::Video => "Video",
    }
}

/// The ⋮ menu for a media item.
pub fn media_menu(app: &App, m: &Media) -> Vec<MenuItem> {
    let b = || Box::new(m.clone());
    let mut items = Vec::new();
    let item = |label: &str, icon: &'static Icon, action: MenuAction| MenuItem {
        label: label.into(),
        icon: Some(icon),
        action,
        checked: false,
    };
    if !m.collection() {
        items.push(item("Play", &ic::PLAY, MenuAction::Play(b())));
        if m.kind == MediaKind::Video {
            items.push(item(
                "Play as audio",
                &ic::MUSIC,
                MenuAction::PlayAudio(b()),
            ));
            items.push(item("Preview", &ic::MINI, MenuAction::Preview(b())));
        }
        items.push(item("Add to queue", &ic::QUEUE, MenuAction::Enqueue(b())));
    }
    let saved = app.saved_urls.contains(&m.url);
    items.push(item(
        if saved { "Remove from saved" } else { "Save" },
        if saved {
            &ic::BOOKMARK_FILLED
        } else {
            &ic::BOOKMARK
        },
        MenuAction::Save(b()),
    ));
    if m.kind != MediaKind::Station && !m.live {
        if !m.collection() {
            items.push(item(
                "Download video",
                &ic::DOWNLOAD,
                MenuAction::Download(b(), false),
            ));
        }
        items.push(item(
            "Download audio",
            &ic::MUSIC,
            MenuAction::Download(b(), true),
        ));
        if !m.collection() {
            for p in app.library.playlists.iter().take(6) {
                items.push(item(
                    &format!("Add to “{}”", p.title),
                    &ic::PLUS,
                    MenuAction::AddToPlaylist(b(), p.id.to_string()),
                ));
            }
            items.push(item(
                "New offline playlist…",
                &ic::PLUS,
                MenuAction::NewPlaylistWith(b()),
            ));
        }
    }
    if !m.channel_id.is_empty() {
        items.push(item(
            "Go to channel",
            &ic::USER,
            MenuAction::OpenChannel(b()),
        ));
    }
    items.push(item(
        "Copy link",
        &ic::LINK,
        MenuAction::CopyLink(m.url.clone()),
    ));
    items.push(item(
        "Open in browser",
        &ic::EXTERNAL,
        MenuAction::OpenInBrowser(m.url.clone()),
    ));
    items
}

pub fn open_menu(s: &mut App, anchor: String, items: Vec<MenuItem>) {
    if s.menu.as_ref().is_some_and(|m| m.anchor == anchor) {
        s.menu = None;
    } else {
        s.menu = Some(MenuState { anchor, items });
    }
}

/// Where a card sits, for its keys, context and artwork shape.
#[derive(Clone)]
pub struct CardCtx {
    pub key: String,
    pub width: f32,
    pub tab: crate::app::Tab,
    pub siblings: std::rc::Rc<Vec<Media>>,
}

/// An artwork-led media card: thumbnail with duration, title, creator and meta. It lifts on
/// hover, shows its ⋮ button, and (for videos, when enabled) plays a muted preview in place.
pub fn media_card(app: &App, m: &Media, cx: &CardCtx) -> El {
    let t = &app.theme;
    let square = matches!(
        m.kind,
        MediaKind::Song | MediaKind::Album | MediaKind::Artist | MediaKind::Station
    );
    let round = matches!(m.kind, MediaKind::Artist | MediaKind::Channel);
    let size = if square { Size::SQUARE } else { Size::CARD };
    let art_h = if square {
        cx.width
    } else {
        cx.width * 9.0 / 16.0
    };
    let radius = if round { art_h / 2.0 } else { sz::R_CARD };
    let key = cx.key.clone();
    let mut art = el()
        .key(format!("{key}-art"))
        .w(cx.width)
        .h(art_h)
        .radius(radius)
        .clip_rounded(radius)
        .fill(t.raised)
        .matched(format!("art-{}", m.url));
    if app.config.thumbnails {
        if let Some(img) = app.images.get(&thumb_url(m, size), size) {
            art = art.child(
                el().w_full()
                    .h_full()
                    .pixels_fit(img.pixels(), rune::SurfaceFit::Fill)
                    .transition(Transition::opacity()),
            );
        } else {
            art = art.child(
                el().w_full()
                    .h_full()
                    .center()
                    .child(icon(placeholder_icon(m.kind), 30.0).color(t.faint)),
            );
        }
    } else {
        art = art.child(
            el().w_full()
                .h_full()
                .center()
                .child(icon(placeholder_icon(m.kind), 30.0).color(t.faint)),
        );
    }
    if let Some(p) = crate::player::preview::surface_for(app, &m.url) {
        art = art.child(
            el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                .native_surface(p, rune::SurfaceFit::Fill)
                .transition(Transition::opacity()),
        );
    }
    if let Some(d) = duration_text(m).filter(|_| app.config.card_info.duration) {
        let live = m.live;
        art = art.child(el().pin(None, None, Some(8.0), Some(8.0)).child(badge(
            &d,
            if live {
                t.error
            } else {
                Color::BLACK.alpha(0.72)
            },
            Color::WHITE,
        )));
    }
    if m.collection() && !round {
        art = art.child(el().pin(Some(8.0), None, None, Some(8.0)).child(badge(
            kind_label(m.kind),
            Color::BLACK.alpha(0.72),
            Color::WHITE,
        )));
    }
    let more_key = format!("{key}-more");
    let menu_items = media_menu(app, m);
    let more = el()
        .key(more_key.clone())
        .pin(None, Some(8.0), Some(8.0), None)
        .size(32.0, 32.0)
        .radius(16.0)
        .center()
        .fill(Color::BLACK.alpha(0.55))
        .group_opacity(0.0, 1.0)
        .press_scale(0.9)
        .role("button")
        .label("More actions")
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut App| open_menu(s, more_key.clone(), menu_items.clone()))
        .child(icon(&ic::MORE, 16.0).color(Color::WHITE));
    art = art.child(more);

    let title = text(&m.title).font(ty::TITLE).color(t.text).max_lines(2);
    let meta = meta_line(app, m);
    let mut words = col().gap(4.0).w_full().shrink().child(title);
    if !meta.is_empty() {
        words = words.child(text(meta).font(ty::SMALL).color(t.muted).max_lines(1));
    }
    if app.config.card_info.description && !m.description.is_empty() {
        words = words.child(
            text(&m.description)
                .font(ty::SMALL)
                .color(t.faint)
                .max_lines(2),
        );
    }
    let mut info = row().w(cx.width).gap(10.0).pt(10.0);
    if m.kind == MediaKind::Video && !m.collection() {
        if let Some(avatar) = Some(m.channel_thumbnail.as_str())
            .filter(|u| !u.is_empty())
            .and_then(|u| app.images.get(u, Size::AVATAR))
        {
            info = info.child(
                el().size(32.0, 32.0)
                    .radius(16.0)
                    .clip_rounded(16.0)
                    .fill(t.raised)
                    .pixels_fit(avatar.pixels(), rune::SurfaceFit::Fill),
            );
        } else if !m.creator.is_empty() {
            let initial: String = m.creator.chars().take(1).collect::<String>().to_uppercase();
            info = info.child(
                el().size(32.0, 32.0)
                    .radius(16.0)
                    .fill(t.accent_soft)
                    .center()
                    .child(text(initial).font(ty::SMALL_STRONG).color(t.accent)),
            );
        }
    }
    info = info.child(words);

    let media = m.clone();
    let siblings = cx.siblings.clone();
    let tab = cx.tab;
    let hover_media = m.clone();
    col()
        .key(key)
        .w(cx.width)
        .hover_group()
        .role("listitem")
        .label(m.title.as_str())
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .press_scale(0.975)
        .on_press(move |s: &mut App| crate::player::actions::open_item(s, tab, &media, &siblings))
        .on_hover_change(move |s: &mut App, inside: bool, tx: &mut Tx| {
            crate::player::preview::hover(s, &hover_media, inside, tx)
        })
        .child(
            el().w(cx.width)
                .h(art_h)
                .radius(radius)
                .shadow(0.0, 0.0, t.shadow)
                .child(art),
        )
        .child(info)
}

fn placeholder_icon(k: MediaKind) -> &'static Icon {
    match k {
        MediaKind::Song | MediaKind::Album => &ic::MUSIC,
        MediaKind::Station => &ic::RADIO,
        MediaKind::Artist | MediaKind::Channel => &ic::USER,
        MediaKind::Playlist => &ic::QUEUE,
        MediaKind::Video => &ic::PLAY,
    }
}

/// A compact row for lists: queue, history, songs, stations, search results in list mode.
pub fn media_row(app: &App, m: &Media, key: &str, trailing: Option<El>) -> El {
    let t = &app.theme;
    let square = matches!(
        m.kind,
        MediaKind::Song | MediaKind::Album | MediaKind::Station | MediaKind::Artist
    );
    let (w, h) = if square { (52.0, 52.0) } else { (92.0, 52.0) };
    let size = if square { Size::SQUARE } else { Size::CARD };
    let mut art = el()
        .w(w)
        .h(h)
        .radius(8.0)
        .clip_rounded(8.0)
        .fill(t.raised)
        .center();
    if let Some(img) = app
        .images
        .get(&thumb_url(m, size), size)
        .filter(|_| app.config.thumbnails)
    {
        art = art.child(
            el().w_full()
                .h_full()
                .pixels_fit(img.pixels(), rune::SurfaceFit::Fill),
        );
    } else {
        art = art.child(icon(placeholder_icon(m.kind), 20.0).color(t.faint));
    }
    let playing = app
        .player
        .now
        .as_ref()
        .is_some_and(|n| n.media.url == m.url);
    let meta = meta_line(app, m);
    let mut words = col().grow(1.0).shrink().gap(2.0).child(
        text(&m.title)
            .font(ty::BODY_STRONG)
            .color(if playing { t.accent } else { t.text })
            .max_lines(1),
    );
    if !meta.is_empty() {
        words = words.child(text(meta).font(ty::SMALL).color(t.muted).max_lines(1));
    }
    let more_key = format!("{key}-more");
    let menu_items = media_menu(app, m);
    let mut r = row()
        .key(key.to_owned())
        .label(m.title.as_str())
        .w_full()
        .px(10.0)
        .py(8.0)
        .gap(14.0)
        .center_y()
        .radius(12.0)
        .hover(t.hover)
        .hover_group()
        .fill(if playing {
            t.accent_soft
        } else {
            Color::TRANSPARENT
        })
        .child(art)
        .child(words);
    if let Some(d) = duration_text(m) {
        r = r.child(text(d).font(ty::NUM).color(t.muted));
    }
    if let Some(tr) = trailing {
        r = r.child(tr);
    }
    r.child(
        el().key(more_key.clone())
            .size(32.0, 32.0)
            .radius(16.0)
            .center()
            .group_opacity(0.0, 1.0)
            .hover(t.hover)
            .role("button")
            .label("More actions")
            .on_press(move |s: &mut App| open_menu(s, more_key.clone(), menu_items.clone()))
            .child(icon(&ic::MORE, 16.0).color(t.muted)),
    )
}

/// The open menu, anchored to its trigger; drawn in the root's overlay layer.
pub fn menu_layer(app: &App) -> Option<El> {
    let m = app.menu.as_ref()?;
    let t = &app.theme;
    let mut list = col()
        .key("menu")
        .w(260.0)
        .padding(6.0)
        .gap(1.0)
        .radius(14.0)
        .fill(t.overlay.alpha(0.92))
        .backdrop_blur(24.0)
        .border(1.0, t.border)
        .shadow(18.0, 48.0, t.shadow)
        .role("menu")
        .focus_group();
    for (i, item) in m.items.iter().enumerate() {
        let action = item.action.clone();
        let mut r = row()
            .key(format!("menu-{i}"))
            .h(36.0)
            .px(10.0)
            .gap(12.0)
            .center_y()
            .radius(8.0)
            .hover(t.hover)
            .focus_highlight(t.hover)
            .role("menuitem")
            .cursor(rune::Cursor::Pointer);
        if let Some(g) = item.icon {
            r = r.child(icon(g, 16.0).color(t.muted));
        }
        r = r.child(
            text(&item.label)
                .font(ty::BODY)
                .color(t.text)
                .grow(1.0)
                .truncate(),
        );
        if item.checked {
            r = r.child(icon(&ic::CHECK, 15.0).color(t.accent));
        }
        list = list
            .child(r.on_press_tx(move |s: &mut App, tx: &mut Tx| s.run_menu(action.clone(), tx)));
    }
    Some(
        list.anchored(m.anchor.clone(), Side::Bottom, Align::End, 6.0)
            .transition(Transition::scale(0.94).combined(Transition::opacity()))
            .modal(true, |s: &mut App| s.menu = None),
    )
}

/// Toasts, bottom-centre, stacked.
pub fn toasts(app: &App, bottom: f32) -> El {
    let t = &app.theme;
    let mut stack = col()
        .key("toasts")
        .pin(Some(0.0), None, Some(0.0), Some(bottom))
        .items_center()
        .gap(8.0);
    for toast in &app.toasts {
        let accent = match toast.kind {
            ToastKind::Info => t.accent,
            ToastKind::Success => t.success,
            ToastKind::Error => t.error,
        };
        let glyph = match toast.kind {
            ToastKind::Info => &ic::INFO,
            ToastKind::Success => &ic::CHECK,
            ToastKind::Error => &ic::WARNING,
        };
        let mut r = row()
            .key(format!("toast-{}", toast.id))
            .min_h(44.0)
            .max_w(560.0)
            .pl(14.0)
            .pr(8.0)
            .py(6.0)
            .gap(12.0)
            .center_y()
            .radius(sz::R_PILL)
            .fill(t.overlay.alpha(0.9))
            .backdrop_blur(20.0)
            .border(1.0, t.border)
            .shadow(10.0, 30.0, t.shadow)
            .role("status")
            .live_region(rune::LiveRegion::Polite)
            .transition(
                Transition::offset(0.0, 24.0)
                    .combined(Transition::scale(0.9))
                    .combined(Transition::opacity()),
            )
            .child(icon(glyph, 16.0).color(accent))
            .child(
                text(&toast.text)
                    .font(ty::BODY)
                    .color(t.text)
                    .shrink()
                    .max_lines(2),
            );
        if let Some((label, _)) = &toast.action {
            let id = toast.id;
            r = r.child(
                row()
                    .h(30.0)
                    .px(12.0)
                    .center()
                    .radius(sz::R_PILL)
                    .fill(t.accent_soft)
                    .hover(t.hover)
                    .press_scale(0.94)
                    .cursor(rune::Cursor::Pointer)
                    .on_press_tx(move |s: &mut App, tx: &mut Tx| s.run_toast(id, tx))
                    .child(text(label).font(ty::SMALL_STRONG).color(t.accent)),
            );
        } else {
            r = r.pr(16.0);
        }
        stack = stack.child(el().child(r));
    }
    stack
}

/// Kit text field bound to an `App` field.
pub fn field(
    t: &Theme,
    edit: &rune::TextEdit,
    placeholder: &str,
    access: fn(&mut App) -> &mut rune::TextEdit,
) -> El {
    rune::text_input(edit, ty::BODY)
        .single_line()
        .placeholder(placeholder, t.faint)
        .on_edit(access)
        .color(t.text)
        .py(10.0)
        .px(14.0)
        .radius(10.0)
        .fill(t.raised)
        .border(1.0, t.border)
}

pub fn spacer_el() -> El {
    spacer()
}

/// A labelled value: "Quality · 1080p" rows inside player menus.
pub fn value_row(_t: &Theme, glyph: &'static Icon, label: &str, value: &str) -> El {
    row()
        .h(40.0)
        .px(12.0)
        .gap(12.0)
        .center_y()
        .radius(10.0)
        .hover(Color::WHITE.alpha(0.08))
        .cursor(rune::Cursor::Pointer)
        .child(icon(glyph, 17.0).color(Color::WHITE.alpha(0.85)))
        .child(text(label).font(ty::BODY).color(Color::WHITE).grow(1.0))
        .child(text(value).font(ty::SMALL).color(Color::WHITE.alpha(0.6)))
        .child(icon(&ic::FORWARD, 14.0).color(Color::WHITE.alpha(0.5)))
        .label(label)
        .role("menuitem")
        .focusable(true)
        .key(format!("vr-{label}"))
        .fill(Color::TRANSPARENT)
        .motion(motion::SNAP)
}
