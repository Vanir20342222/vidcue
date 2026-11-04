//! "A new version is available" popup: startup check, buttons and rendering.
use crate::app::{App, Modal, UiEvent};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use vidcue_services::update::{self, Release};

/// Button labels, in order. Index 0 installs.
pub const BUTTONS: [&str; 3] = ["Update now", "Later", "Skip this version"];

impl App {
    /// Look for a newer release in the background. `manual` comes from F2 → Check for
    /// updates and reports "up to date" or errors; the startup check stays silent.
    pub fn check_for_updates(&mut self, manual: bool) {
        if self.offline {
            if manual {
                self.status = "Offline mode · update check skipped".into();
            }
            return;
        }
        let proxy = self.config.proxy.clone();
        if manual {
            self.status = "Checking GitHub for a new release…".into();
        }
        self.tasks.spawn(async move {
            let result = update::check(
                update::API_BASE,
                proxy.as_deref(),
                env!("CARGO_PKG_VERSION"),
            )
            .await;
            UiEvent::Update { manual, result }
        });
        self.redraw = true;
    }

    pub fn update_found(&mut self, manual: bool, result: Result<Option<Release>>) {
        match result {
            Ok(Some(release)) => {
                let skipped = self.config.skipped_update.as_deref() == Some(&release.version);
                if skipped && !manual {
                    return;
                }
                if self.modal.is_none() || manual {
                    self.modal = Some(Modal::Update {
                        release: Box::new(release),
                        selected: 0,
                    });
                } else {
                    self.status = format!(
                        "Vidcue {} is available · F2 → Check for a new Vidcue version",
                        release.version
                    );
                }
            }
            Ok(None) if manual => {
                self.status = format!("Vidcue {} is up to date", env!("CARGO_PKG_VERSION"))
            }
            Err(error) if manual => {
                self.status = vidcue_services::process::diagnostic(&error.to_string())
            }
            _ => {}
        }
        self.redraw = true;
    }

    pub fn update_key(&mut self, release: Box<Release>, mut selected: usize, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('l') => return,
            KeyCode::Left | KeyCode::BackTab | KeyCode::Up => {
                selected = (selected + BUTTONS.len() - 1) % BUTTONS.len()
            }
            KeyCode::Right | KeyCode::Tab | KeyCode::Down => {
                selected = (selected + 1) % BUTTONS.len()
            }
            KeyCode::Char('u') => return self.choose_update(*release, 0),
            KeyCode::Char('s') => return self.choose_update(*release, 2),
            KeyCode::Enter => return self.choose_update(*release, selected),
            _ => {}
        }
        self.modal = Some(Modal::Update { release, selected });
        self.redraw = true;
    }

    fn choose_update(&mut self, release: Release, button: usize) {
        match button {
            0 => {
                self.status = format!("Installing Vidcue {}…", release.version);
                self.run_update = Some(release);
            }
            2 => {
                let status = format!(
                    "Skipped Vidcue {} · F2 → Check for a new Vidcue version",
                    release.version
                );
                let mut config = self.config.clone();
                config.skipped_update = Some(release.version);
                self.save_config(config);
                self.status = status;
            }
            _ => {}
        }
        self.redraw = true;
    }
}
