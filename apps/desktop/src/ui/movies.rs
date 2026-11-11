//! Movies (alpha): a cinematic page over the Cinecat backend. A full-bleed hero for the featured
//! title, rows of poster cards, an optional TMDB search, an "Open by TMDB id" panel, and a
//! frosted detail sheet with the season/episode picker and the provider check list.

use rune::{Color, Cx, Cycle, El, ScrollSpec, SurfaceFit, Transition, col, el, icon, row, text};

use crate::app::App;
use crate::icons as ic;
use crate::images::Size;
use crate::movies::{self, Phase, Title, tmdb};
use crate::theme::{Theme, sz, ty};
use crate::ui::widgets::{self, Kind, button, chip, icon_button};

const CARD_W: f32 = 168.0;
const CARD_H: f32 = 252.0;
const MESH_CARDS: usize = 8;

pub fn view(app: &App, cx: &mut Cx<App>) -> El {
    let width = cx
        .layout_size("movies-scroll")
        .map(|s| s[0])
        .unwrap_or(1180.0);
    let m = &app.movies;
    let mut body = col()
        .w_full()
        .pt(sz::TOPBAR + 24.0)
        .pb(sz::DOCK + 48.0)
        .gap(28.0)
        .child(header(app));
    if !app.prefs.alpha_notice_accepted {
        body = body.child(notice(app));
    } else {
        body = body.child(hero(app, width)).child(finder(app));
        let keyed = !app.prefs.tmdb_key.trim().is_empty();
        if keyed {
            if m.searching || m.searched.is_some() || m.search_error.is_some() {
                body = body.child(search_row(app));
            }
            body = body.child(trending_row(app));
        }
        let sub = "Titles that resolved when the Cinecat pool was last checked. Availability can change at any time.";
        body = body.child(poster_row(
            app,
            "shelf",
            "Curated picks",
            Some(sub),
            &m.shelf,
        ));
        if !keyed {
            body = body.child(
                row().px(sz::PAGE_PAD).gap(10.0).center_y()
                    .child(icon(&ic::KEY, 16.0).color(app.theme.faint))
                    .child(text("Add a TMDB API key in Settings → Alpha for posters, trending titles and search.").font(ty::SMALL).color(app.theme.muted)),
            );
        }
    }
    let scroller = col()
        .key("movies-scroll")
        .w_full()
        .h_full()
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(body);
    let mut page = el().key("movies-page").w_full().h_full().child(scroller);
    if app.prefs.alpha_notice_accepted
        && let Some(sel) = &m.selected
    {
        page = page.child(sheet(app, sel));
    }
    page
}

fn header(app: &App) -> El {
    let t = &app.theme;
    col().px(sz::PAGE_PAD).gap(8.0)
        .child(row().gap(12.0).center_y().child(text("Movies").font(ty::H1).color(t.text)).child(crate::ui::sidebar::alpha_badge_for(t)))
        .child(text("Alpha: streams come from third-party providers through Cinecat. Titles may not play, and availability changes without notice.").font(ty::SMALL).color(t.muted).wrap())
}

fn notice(app: &App) -> El {
    let t = &app.theme;
    widgets::panel(t).mx(sz::PAGE_PAD).p_notice().child(
        col().gap(14.0).max_w(640.0)
            .child(row().gap(10.0).center_y().child(icon(&ic::WARNING, 20.0).color(t.warning)).child(text("Before you start").font(ty::H2).color(t.text)))
            .child(text("Movies is an alpha feature. It asks Cinecat's third-party providers for a stream and plays it in Vidcue. Vidcue does not host or vouch for any of it, many titles will fail, and you are responsible for using it where that is lawful.").font(ty::BODY).color(t.muted).wrap())
            .child(button(t, "I understand, show Movies", Some(&ic::CHECK), Kind::Primary).key("movies-accept").on_press(|s: &mut App| movies::accept_notice(s))),
    )
}

/// Panels in this page pad themselves; `El` has no `.p()`.
trait Pad {
    fn p_notice(self) -> Self;
}
impl Pad for El {
    fn p_notice(self) -> Self {
        self.padding(24.0)
    }
}

// ---- generated and real artwork ----

/// A seeded gradient standing in for a missing poster. The first few cards get the animated
/// palette mesh; the rest a still gradient, so a long shelf stays cheap to draw.
fn generated_art(seed: u64, animated: bool) -> El {
    let fill = || -> El {
        let a = (seed % 360) as f32 * std::f32::consts::PI / 180.0;
        let c1 = Color::from_oklab([0.34, 0.09 * a.cos(), 0.09 * a.sin(), 1.0]);
        let c2 = Color::from_oklab([0.2, 0.06 * (a + 2.0).cos(), 0.06 * (a + 2.0).sin(), 1.0]);
        el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .linear_gradient(1.0 + a * 0.1, [(0.0, c1), (1.0, c2)])
    };
    if animated {
        crate::fx::mesh_preset((seed % 5) as u32, (seed % 997) as u32, 0.06).pin(
            Some(0.0),
            Some(0.0),
            Some(0.0),
            Some(0.0),
        )
    } else {
        fill()
    }
}

fn poster_card(app: &App, row_id: &str, i: usize, title: &Title) -> El {
    let t = &app.theme;
    let key = format!("mv-{row_id}-{}-{}", title.tmdb, i);
    let img = app
        .images
        .get(&tmdb::image_url(&title.poster, "w342"), Size::POSTER);
    let mut art = el()
        .key(format!("{key}-art"))
        .w(CARD_W)
        .h(CARD_H)
        .radius(sz::R_CARD)
        .clip_rounded(sz::R_CARD)
        .fill(t.raised)
        .matched(format!("movie-art-{row_id}-{}", title.tmdb));
    art = match &img {
        Some(img) => art.child(
            el().w_full()
                .h_full()
                .pixels_fit(img.pixels(), SurfaceFit::Fill)
                .transition(Transition::opacity()),
        ),
        None => art
            .child(generated_art(title.tmdb, i < MESH_CARDS))
            .child(
                el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                    .gradient(Color::TRANSPARENT, Color::BLACK.alpha(0.6)),
            )
            .child(
                el().pin(Some(12.0), None, Some(12.0), Some(12.0)).child(
                    text(&title.title)
                        .font(ty::H3)
                        .color(Color::WHITE)
                        .max_lines(4)
                        .wrap(),
                ),
            ),
    };
    art = art
        .child(
            el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                .radius(sz::R_CARD)
                .border(2.0, t.accent)
                .group_opacity(0.0, 1.0)
                .inert(),
        )
        .child(
            el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                .center()
                .group_opacity(0.0, 1.0)
                .inert()
                .child(
                    el().size(46.0, 46.0)
                        .radius(23.0)
                        .center()
                        .fill(Color::BLACK.alpha(0.55))
                        .child(icon(&ic::PLAY, 20.0).color(Color::WHITE)),
                ),
        );
    let meta = if !title.year.is_empty() {
        title.year.clone()
    } else if title.tv {
        "Series".into()
    } else {
        "Film".into()
    };
    let meta = match (title.tv, title.season, title.episode) {
        (true, Some(s), Some(e)) if title.year.is_empty() => format!("Series · S{s}E{e}"),
        _ => meta,
    };
    let (open, rid) = (title.clone(), row_id.to_owned());
    col()
        .key(key)
        .w(CARD_W)
        .gap(8.0)
        .hover_group()
        .role("listitem")
        .label(title.title.as_str())
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .press_scale(0.97)
        .on_press(move |s: &mut App| {
            s.movies.row = rid.clone();
            movies::open_title(s, open.clone());
        })
        .child(
            el().w(CARD_W)
                .h(CARD_H)
                .radius(sz::R_CARD)
                .shadow(10.0, 24.0, t.shadow)
                .child(art),
        )
        .child(
            col()
                .gap(2.0)
                .w_full()
                .child(
                    text(&title.title)
                        .font(ty::TITLE)
                        .color(t.text)
                        .max_lines(2),
                )
                .child(text(meta).font(ty::SMALL).color(t.muted).max_lines(1)),
        )
}

fn poster_row(app: &App, id: &str, title: &str, subtitle: Option<&str>, items: &[Title]) -> El {
    let t = &app.theme;
    let cards = items
        .iter()
        .enumerate()
        .map(|(i, it)| poster_card(app, id, i, it).enter_delay((i.min(10) as f32) * 0.03));
    col()
        .gap(14.0)
        .w_full()
        .child(
            el().px(sz::PAGE_PAD)
                .child(widgets::section_header(t, title, subtitle)),
        )
        .child(
            el().key(format!("mv-row-{id}")).w_full().scroll_x().child(
                row()
                    .gap(16.0)
                    .px(sz::PAGE_PAD)
                    .pb(10.0)
                    .items_start()
                    .children(cards),
            ),
        )
}

fn skeleton_row(app: &App) -> El {
    let t = &app.theme;
    row().gap(16.0).px(sz::PAGE_PAD).children((0..8).map(|i| {
        el().key(format!("mv-sk-{i}"))
            .w(CARD_W)
            .h(CARD_H)
            .radius(sz::R_CARD)
            .fill(t.raised)
            .cycle(Cycle {
                phase: i as f32 * 0.08,
                ..widgets::PULSE
            })
    }))
}

fn trending_row(app: &App) -> El {
    let t = &app.theme;
    let s = &app.movies.trending;
    if !s.items.is_empty() {
        return poster_row(
            app,
            "trending",
            "Trending this week",
            Some("From TMDB"),
            &s.items,
        );
    }
    let head = el().px(sz::PAGE_PAD).child(widgets::section_header(
        t,
        "Trending this week",
        Some("From TMDB"),
    ));
    if let Some(e) = &s.error {
        return col().gap(10.0).child(head).child(
            row()
                .px(sz::PAGE_PAD)
                .gap(12.0)
                .center_y()
                .child(text(e).font(ty::SMALL).color(t.error))
                .child(
                    button(t, "Retry", Some(&ic::REFRESH), Kind::Secondary)
                        .key("mv-trending-retry")
                        .on_press(|s: &mut App| movies::load_trending(s)),
                ),
        );
    }
    col().gap(14.0).child(head).child(skeleton_row(app))
}

fn search_row(app: &App) -> El {
    let t = &app.theme;
    let m = &app.movies;
    let q = m.searched.clone().unwrap_or_default();
    if m.searching {
        return col()
            .gap(14.0)
            .child(
                el().px(sz::PAGE_PAD)
                    .child(widgets::section_header(t, "Searching…", None)),
            )
            .child(skeleton_row(app));
    }
    if let Some(e) = &m.search_error {
        return el()
            .px(sz::PAGE_PAD)
            .child(text(e).font(ty::SMALL).color(t.error));
    }
    if m.results.is_empty() {
        return el().px(sz::PAGE_PAD).child(
            text(format!("No films or series found for “{q}”."))
                .font(ty::BODY)
                .color(t.muted),
        );
    }
    poster_row(
        app,
        "results",
        &format!("Results for “{q}”"),
        None,
        &m.results,
    )
}

// ---- the hero ----

fn hero(app: &App, width: f32) -> El {
    let t = &app.theme;
    let Some(f) = app.movies.featured_title().cloned() else {
        return el();
    };
    let w = (width - sz::PAGE_PAD * 2.0).max(320.0);
    let h = (width * 0.3).clamp(320.0, 440.0);
    // Without a backdrop (no TMDB key), the poster blurred wide makes the stage.
    let (back, blur) = match app
        .images
        .get(&tmdb::image_url(&f.backdrop, "w1280"), Size::HERO)
    {
        Some(i) => (Some(i), 10.0),
        None => (
            app.images
                .get(&tmdb::image_url(&f.poster, "w342"), Size::POSTER),
            48.0,
        ),
    };
    let mut stage = el()
        .key("movies-hero")
        .w(w)
        .h(h)
        .radius(24.0)
        .clip_rounded(24.0)
        .fill(t.surface);
    stage = match &back {
        Some(i) => stage.child(
            el().pin(Some(-24.0), Some(-24.0), Some(-24.0), Some(-24.0))
                .pixels_fit(i.pixels(), SurfaceFit::Fill)
                .blur(blur)
                .opacity(if blur > 20.0 { 0.75 } else { 1.0 })
                .transition(Transition::opacity()),
        ),
        None => stage
            .child(crate::fx::mesh(f.tmdb as u32, 0.12).pin(
                Some(0.0),
                Some(0.0),
                Some(0.0),
                Some(0.0),
            ))
            .child(
                crate::fx::aurora(0.8, 0.3)
                    .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                    .opacity(0.7),
            ),
    };
    stage = stage
        .child(
            el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                .linear_gradient(
                    std::f32::consts::FRAC_PI_2,
                    [
                        (0.0, Color::BLACK.alpha(0.78)),
                        (0.6, Color::BLACK.alpha(0.25)),
                        (1.0, Color::TRANSPARENT),
                    ],
                ),
        )
        .child(
            el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                .gradient(Color::TRANSPARENT, Color::BLACK.alpha(0.35)),
        )
        .child(crate::fx::grain(0.06).pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)));
    if let Some(p) = app
        .images
        .get(&tmdb::image_url(&f.poster, "w342"), Size::POSTER)
        && w > 900.0
    {
        stage = stage.child(
            el().key("movies-hero-poster")
                .pin(None, Some(((h - 270.0) / 2.0).max(16.0)), Some(48.0), None)
                .w(180.0)
                .h(270.0)
                .radius(14.0)
                .clip_rounded(14.0)
                .shadow(24.0, 60.0, Color::BLACK.alpha(0.5))
                .border(1.0, Color::WHITE.alpha(0.12))
                .child(
                    el().w_full()
                        .h_full()
                        .pixels_fit(p.pixels(), SurfaceFit::Fill),
                ),
        );
    }
    let meta = tmdb::meta_text(&f);
    let (play, more) = (f.clone(), f.clone());
    let mut words = col()
        .pin(Some(40.0), None, None, Some(36.0))
        .max_w((w * 0.5).max(360.0))
        .gap(12.0)
        .child(
            text("FEATURED")
                .font(ty::CAPTION)
                .color(Color::WHITE.alpha(0.75)),
        )
        .child(
            text(&f.title)
                .font(ty::DISPLAY)
                .color(Color::WHITE)
                .max_lines(2),
        )
        .child(
            text(meta)
                .font(ty::BODY)
                .color(Color::WHITE.alpha(0.78))
                .max_lines(1),
        );
    if !f.overview.is_empty() {
        words = words.child(
            text(&f.overview)
                .font(ty::BODY)
                .color(Color::WHITE.alpha(0.85))
                .wrap()
                .max_lines(3),
        );
    }
    words = words.child(
        row()
            .gap(10.0)
            .pt(8.0)
            .child(
                crate::ui::browse::play_button(app)
                    .key("movies-hero-play")
                    .on_press(move |s: &mut App| movies::play_title(s, &play)),
            )
            .child(
                button(t, "Details", Some(&ic::INFO), Kind::Glass)
                    .key("movies-hero-details")
                    .on_press(move |s: &mut App| {
                        s.movies.row = String::new();
                        movies::open_title(s, more.clone());
                    }),
            )
            .child(
                icon_button(t, &ic::REFRESH, "Another pick", 38.0)
                    .key("movies-hero-next")
                    .on_press(|s: &mut App| movies::next_featured(s)),
            ),
    );
    el().px(sz::PAGE_PAD)
        .child(stage.child(words))
        .transition(Transition::scale(0.98))
}

// ---- search and open by id ----

fn finder(app: &App) -> El {
    let t = &app.theme;
    let m = &app.movies;
    let keyed = !app.prefs.tmdb_key.trim().is_empty();
    let mut r = row().px(sz::PAGE_PAD).gap(16.0).items_start().flex_wrap();
    if keyed {
        let field = widgets::field(
            t,
            &m.search,
            "Search films and series on TMDB",
            |s: &mut App| &mut s.movies.search,
        )
        .key("movies-search")
        .grow(1.0)
        .label("Search TMDB")
        .on_submit(|s: &mut App, tx: &mut rune::Tx| {
            movies::search(s);
            tx.blur();
        });
        r = r.child(
            widgets::panel(t)
                .w(420.0)
                .padding(16.0)
                .gap(10.0)
                .child(text("Search").font(ty::H3).color(t.text))
                .child(
                    row().gap(8.0).child(field).child(
                        button(t, "Search", Some(&ic::SEARCH), Kind::Secondary)
                            .key("movies-search-go")
                            .on_press(|s: &mut App| movies::search(s)),
                    ),
                ),
        );
    }
    let mut ids = row().gap(8.0).items_end().flex_wrap();
    ids = ids
        .child(widgets::segmented(
            t,
            "mv-id-kind",
            &["Film", "Series"],
            m.id_tv as usize,
            |s: &mut App, i| s.movies.id_tv = i == 1,
        ))
        .child(
            widgets::field(t, &m.id_tmdb, "TMDB id, e.g. 27205", |s: &mut App| {
                &mut s.movies.id_tmdb
            })
            .key("mv-id-tmdb")
            .w(170.0)
            .label("TMDB id")
            .on_submit(|s: &mut App, tx: &mut rune::Tx| {
                movies::open_by_id(s);
                tx.blur();
            }),
        );
    if m.id_tv {
        ids = ids
            .child(
                widgets::field(t, &m.id_season, "Season", |s: &mut App| {
                    &mut s.movies.id_season
                })
                .key("mv-id-season")
                .w(84.0)
                .label("Season"),
            )
            .child(
                widgets::field(t, &m.id_episode, "Episode", |s: &mut App| {
                    &mut s.movies.id_episode
                })
                .key("mv-id-episode")
                .w(84.0)
                .label("Episode"),
            );
    }
    ids = ids.child(
        button(t, "Open", Some(&ic::PLAY), Kind::Secondary)
            .key("mv-id-open")
            .on_press(|s: &mut App| movies::open_by_id(s)),
    );
    let mut panel = widgets::panel(t)
        .grow(1.0)
        .min_w(420.0)
        .padding(16.0)
        .gap(10.0)
        .child(text("Open by TMDB id").font(ty::H3).color(t.text))
        .child(ids);
    if let Some(e) = &m.id_error {
        panel = panel.child(text(e).font(ty::SMALL).color(t.error));
    }
    r.child(panel)
}

// ---- the detail sheet ----

fn sheet(app: &App, sel: &Title) -> El {
    let t = &app.theme;
    let m = &app.movies;
    let back = app
        .images
        .get(&tmdb::image_url(&sel.backdrop, "w1280"), Size::HERO);
    let poster = app
        .images
        .get(&tmdb::image_url(&sel.poster, "w342"), Size::POSTER);

    let mut banner = el().w_full().h(190.0).clip_rounded(0.0).fill(t.raised);
    banner = match &back {
        Some(i) => banner.child(
            el().w_full()
                .h_full()
                .pixels_fit(i.pixels(), SurfaceFit::Fill)
                .transition(Transition::opacity()),
        ),
        None => banner
            .child(crate::fx::mesh(sel.tmdb as u32, 0.1).pin(
                Some(0.0),
                Some(0.0),
                Some(0.0),
                Some(0.0),
            ))
            .child(crate::fx::grain(0.05).pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))),
    };
    banner = banner.child(
        el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .gradient(Color::TRANSPARENT, t.overlay),
    );
    let close = icon_button(t, &ic::CLOSE, "Close", 34.0)
        .key("movies-sheet-close")
        .fill(Color::BLACK.alpha(0.45))
        .on_press(|s: &mut App| movies::close_detail(s));
    banner = banner.child(el().pin(None, Some(12.0), Some(12.0), None).child(close));

    let mut art = el()
        .key("movies-sheet-art")
        .w(110.0)
        .h(165.0)
        .radius(12.0)
        .clip_rounded(12.0)
        .fill(t.raised)
        .shadow(14.0, 30.0, t.shadow);
    if !m.row.is_empty() {
        art = art.matched(format!("movie-art-{}-{}", m.row, sel.tmdb));
    }
    art = match &poster {
        Some(p) => art.child(
            el().w_full()
                .h_full()
                .pixels_fit(p.pixels(), SurfaceFit::Fill),
        ),
        None => art.child(generated_art(sel.tmdb, true)),
    };
    let head = row()
        .px(24.0)
        .gap(16.0)
        .items_end()
        .child(art.mt(-70.0))
        .child(
            col()
                .grow(1.0)
                .shrink()
                .gap(4.0)
                .pb(4.0)
                .child(text(&sel.title).font(ty::H1).color(t.text).max_lines(3))
                .child(
                    text(tmdb::meta_text(sel))
                        .font(ty::SMALL)
                        .color(t.muted)
                        .wrap()
                        .max_lines(2),
                ),
        );

    let mut body = col().px(24.0).gap(18.0).pb(24.0);
    if !sel.overview.is_empty() {
        body = body.child(text(&sel.overview).font(ty::BODY).color(t.muted).wrap());
    } else if app.prefs.tmdb_key.trim().is_empty() {
        body = body.child(text(format!("TMDB id {}. Add a TMDB API key in Settings → Alpha to see the synopsis, poster and episode lists.", sel.tmdb)).font(ty::SMALL).color(t.faint).wrap());
    }
    if sel.tv {
        body = body.child(episode_picker(app));
    }
    body = body.child(actions(app, sel)).child(progress(app));
    body = body.child(text("Alpha: a title can fail even when the check passes. Providers may be slow, blocked or removed without notice.").font(ty::SMALL).color(t.faint).wrap());

    let panel = col()
        .key("movies-sheet")
        .pin(
            None,
            Some(sz::TOPBAR + 8.0),
            Some(20.0),
            Some(sz::DOCK + 20.0),
        )
        .w(480.0)
        .radius(sz::R_PANEL + 6.0)
        .clip_rounded(sz::R_PANEL + 6.0)
        .fill(t.overlay.alpha(0.9))
        .backdrop_blur(28.0)
        .border(1.0, t.border)
        .shadow(24.0, 64.0, t.shadow)
        .role("dialog")
        .label(sel.title.as_str())
        .transition(Transition::offset(48.0, 0.0).combined(Transition::opacity()))
        .child(banner)
        .child(
            col()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .gap(16.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(col().gap(16.0).child(head).child(body)),
        );
    let scrim = el()
        .key("movies-scrim")
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .fill(t.scrim.alpha(0.3))
        .transition(Transition::opacity())
        .on_press(|s: &mut App| movies::close_detail(s));
    el().key("movies-sheet-layer")
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .child(scrim)
        .child(panel)
}

fn actions(app: &App, sel: &Title) -> El {
    let t = &app.theme;
    let m = &app.movies;
    let _ = sel;
    let mut r = row().gap(10.0).center_y();
    match &m.phase {
        Phase::Checking => {
            r = r.child(
                button(t, "Cancel", Some(&ic::CLOSE), Kind::Secondary)
                    .key("movies-cancel")
                    .on_press(|s: &mut App| movies::cancel_check(s)),
            );
        }
        Phase::Failed { .. } => {
            r = r.child(
                button(t, "Try again", Some(&ic::REFRESH), Kind::Primary)
                    .key("movies-again")
                    .on_press(|s: &mut App| movies::play_selected(s)),
            );
        }
        Phase::Idle => {
            r = r.child(
                crate::ui::browse::play_button(app)
                    .key("movies-sheet-play")
                    .on_press(|s: &mut App| movies::play_selected(s)),
            );
        }
    }
    r
}

fn episode_picker(app: &App) -> El {
    let t = &app.theme;
    let m = &app.movies;
    let mut c = col().gap(10.0);
    if m.seasons.len() > 1 {
        c = c.child(
            el().key("mv-seasons")
                .w_full()
                .scroll_x()
                .child(row().gap(8.0).children(m.seasons.iter().map(|s| {
                    let n = s.number;
                    chip(t, &format!("Season {n}"), n == m.season)
                        .key(format!("mv-season-{n}"))
                        .on_press(move |a: &mut App| movies::pick_season(a, n))
                }))),
        );
    } else {
        c = c.child(stepper(app, "Season", m.season, true));
    }
    if m.episodes.is_empty() {
        c = c.child(stepper(app, "Episode", m.episode, false));
    } else {
        let list = m.episodes.iter().map(|e| {
            let on = e.number == m.episode;
            let n = e.number;
            let mut words = col().grow(1.0).shrink().gap(2.0).child(
                text(if e.name.is_empty() {
                    format!("Episode {n}")
                } else {
                    e.name.clone()
                })
                .font(ty::BODY_STRONG)
                .color(t.text)
                .max_lines(1),
            );
            if let Some(r) = e.runtime {
                words = words.child(text(tmdb::runtime_text(r)).font(ty::SMALL).color(t.muted));
            }
            row()
                .key(format!("mv-ep-{n}"))
                .h(48.0)
                .px(12.0)
                .gap(12.0)
                .center_y()
                .radius(10.0)
                .fill(if on {
                    t.accent_soft
                } else {
                    Color::TRANSPARENT
                })
                .hover(t.hover)
                .press_scale(0.99)
                .role("radio")
                .checked(on)
                .label(format!("Episode {n}"))
                .cursor(rune::Cursor::Pointer)
                .on_press(move |a: &mut App| movies::pick_episode(a, n))
                .child(
                    text(n.to_string())
                        .font(ty::NUM)
                        .color(if on { t.accent } else { t.faint })
                        .w(24.0),
                )
                .child(words)
        });
        c = c.child(
            col()
                .key("mv-episodes")
                .w_full()
                .max_h(224.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .gap(2.0)
                .children(list),
        );
    }
    c
}

fn stepper(app: &App, label: &str, value: u32, season: bool) -> El {
    let t = &app.theme;
    let k = if season { "s" } else { "e" };
    row()
        .gap(10.0)
        .center_y()
        .child(text(label).font(ty::BODY_STRONG).color(t.text).w(72.0))
        .child(
            icon_button(
                t,
                &ic::BACK,
                &format!("Previous {}", label.to_lowercase()),
                34.0,
            )
            .key(format!("mv-step-{k}-dec"))
            .on_press(move |a: &mut App| movies::step(a, season, -1)),
        )
        .child(
            text(value.to_string())
                .font(ty::NUM_BIG)
                .color(t.text)
                .w(52.0)
                .align(rune::Align::Center),
        )
        .child(
            icon_button(
                t,
                &ic::FORWARD,
                &format!("Next {}", label.to_lowercase()),
                34.0,
            )
            .key(format!("mv-step-{k}-inc"))
            .on_press(move |a: &mut App| movies::step(a, season, 1)),
        )
}

// ---- provider progress ----

fn progress(app: &App) -> El {
    let t = &app.theme;
    match &app.movies.phase {
        Phase::Idle => el(),
        Phase::Checking => {
            // One calm status: a spinner, what is happening, an indeterminate bar, and the
            // providers that will be tried, in order.
            let providers = movies::PROVIDERS.join("  ·  ");
            col()
                .key("mv-progress")
                .gap(12.0)
                .p_card(t)
                .child(
                    row()
                        .gap(12.0)
                        .center_y()
                        .child(crate::ui::watch_controls::spinner_sized(t.accent, 22.0))
                        .child(
                            col()
                                .gap(2.0)
                                .child(text("Finding a working stream").font(ty::BODY_STRONG).color(t.text))
                                .child(text("Each provider must deliver real video before it is used. This can take a minute.").font(ty::SMALL).color(t.muted).wrap()),
                        ),
                )
                .child(widgets::shimmer_block(t, 420.0, 6.0, 3.0).w_full())
                .child(text(format!("Trying {providers}")).font(ty::SMALL).color(t.faint))
                .transition(Transition::offset(0.0, 8.0).combined(Transition::opacity()))
        }
        Phase::Failed { message, attempts } => {
            let rows = attempts.iter().enumerate().map(|(i, a)| {
                let (glyph, color) = if a.ok {
                    (&ic::CHECK, t.success)
                } else {
                    (&ic::CLOSE, t.error)
                };
                let mut words = col().grow(1.0).shrink().gap(2.0).child(
                    row()
                        .gap(8.0)
                        .child(text(&a.provider).font(ty::NUM).color(t.text))
                        .child(if a.ms > 0 {
                            text(format!("{:.1}s", a.ms as f64 / 1000.0))
                                .font(ty::SMALL)
                                .color(t.faint)
                        } else {
                            text("").font(ty::SMALL)
                        }),
                );
                if let Some(e) = &a.error {
                    words = words.child(text(e).font(ty::SMALL).color(t.muted).wrap().max_lines(2));
                }
                row()
                    .key(format!("mv-attempt-{i}"))
                    .gap(10.0)
                    .items_start()
                    .child(icon(glyph, 16.0).color(color))
                    .child(words)
                    .enter_delay(i as f32 * 0.04)
            });
            col()
                .key("mv-progress")
                .gap(10.0)
                .p_card(t)
                .child(
                    row()
                        .gap(8.0)
                        .center_y()
                        .child(icon(&ic::WARNING, 18.0).color(t.warning))
                        .child(
                            text("Could not start this title")
                                .font(ty::BODY_STRONG)
                                .color(t.text),
                        ),
                )
                .child(text(message).font(ty::SMALL).color(t.muted).wrap())
                .children(rows)
                .transition(Transition::offset(0.0, 8.0).combined(Transition::opacity()))
        }
    }
}

trait Card {
    fn p_card(self, t: &Theme) -> Self;
}
impl Card for El {
    fn p_card(self, t: &Theme) -> Self {
        self.padding(14.0)
            .radius(12.0)
            .fill(t.surface)
            .border(1.0, t.hairline)
    }
}
