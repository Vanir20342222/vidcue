//! Keyboard shortcuts, YouTube-style for playback, plus app navigation.

use rune::{Key, KeyEvent, Tx};

use crate::app::{App, Route, SettingsSection};
use crate::player::session;

pub fn on_key(app: &mut App, e: &KeyEvent, tx: &mut Tx) -> bool {
    // A focused text field keeps its keys; only Escape leaves it.
    let typing = app.search.focused
        || tx
            .focused()
            .is_some_and(|f| f.contains("field") || f.contains("input"));
    match &e.key {
        Key::Escape => {
            if app.menu.is_some() {
                app.menu = None;
            } else if app.player.menu.is_some() {
                app.player.menu = None;
            } else if app.player.fullscreen {
                app.player.fullscreen = false;
                tx.toggle_full_screen();
            } else if typing {
                tx.blur();
            } else if app.route == Route::Watch {
                app.go_back();
            } else {
                return false;
            }
            true
        }
        Key::Command('l') | Key::Command('k') | Key::Command('f') => {
            tx.focus("search", true);
            true
        }
        Key::Command(',') => {
            app.go_root(Route::Settings(SettingsSection::Appearance));
            true
        }
        Key::Command('[') => {
            app.go_back();
            true
        }
        Key::Command(']') => {
            app.go_forward();
            true
        }
        Key::Command('b') => {
            app.edit_prefs(|p| p.sidebar_compact = !p.sidebar_compact);
            true
        }
        _ if typing => false,
        Key::Text(t) if app.player.now.is_some() => match t.as_str() {
            " " | "k" => {
                app.player.toggle_pause();
                true
            }
            "j" => {
                app.player.seek_by(-app.player.seek_seconds);
                true
            }
            "l" => {
                app.player.seek_by(app.player.seek_seconds);
                true
            }
            "m" => {
                app.player.toggle_mute();
                true
            }
            "f" => {
                if app.route != Route::Watch {
                    app.go(Route::Watch);
                }
                app.player.fullscreen = !app.player.fullscreen;
                tx.toggle_full_screen();
                true
            }
            "t" => {
                app.player.theater = !app.player.theater;
                true
            }
            "c" => {
                let next = match app
                    .player
                    .now
                    .as_ref()
                    .map(|n| (n.caption, n.captions.len()))
                {
                    Some((Some(_), _)) => None,
                    Some((None, n)) if n > 0 => Some(0),
                    _ => None,
                };
                app.player.set_caption(next);
                true
            }
            "N" | "n" => {
                session::next(app, false);
                true
            }
            "P" | "p" => {
                session::previous(app);
                true
            }
            ">" => {
                let s = (app.player.speed + 0.25).min(3.0);
                app.player.set_speed(s);
                app.player.flash = Some((format!("Speed {s}×"), app.clock + 1.2));
                true
            }
            "<" => {
                let s = (app.player.speed - 0.25).max(0.25);
                app.player.set_speed(s);
                app.player.flash = Some((format!("Speed {s}×"), app.clock + 1.2));
                true
            }
            "/" => {
                tx.focus("search", true);
                true
            }
            d if d.len() == 1 && d.chars().all(|c| c.is_ascii_digit()) => {
                if let Some(dur) = app.player.snap.duration {
                    let f = d.parse::<f64>().unwrap_or(0.0) / 10.0;
                    app.player.seek_to(dur * f, true);
                }
                true
            }
            _ => false,
        },
        Key::Text(t) if t == "/" => {
            tx.focus("search", true);
            true
        }
        Key::Left if app.player.now.is_some() && app.route == Route::Watch => {
            app.player.seek_by(-5.0);
            true
        }
        Key::Right if app.player.now.is_some() && app.route == Route::Watch => {
            app.player.seek_by(5.0);
            true
        }
        Key::Up if app.player.now.is_some() && app.route == Route::Watch => {
            let v = app.player.volume + 5.0;
            app.player.set_volume(v);
            app.player.flash = Some((format!("Volume {:.0}%", app.player.volume), app.clock + 1.0));
            true
        }
        Key::Down if app.player.now.is_some() && app.route == Route::Watch => {
            let v = app.player.volume - 5.0;
            app.player.set_volume(v);
            app.player.flash = Some((format!("Volume {:.0}%", app.player.volume), app.clock + 1.0));
            true
        }
        _ => false,
    }
}
