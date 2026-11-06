use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};
use vidcue_core::Config;

#[derive(Clone, Debug)]
pub struct Paths {
    pub config_override: Option<PathBuf>,
    pub xdg_state: PathBuf,
    pub config: PathBuf,
    pub state: PathBuf,
    pub cache: PathBuf,
    pub legacy_data: PathBuf,
    pub home: PathBuf,
}
impl Paths {
    pub fn discover() -> Result<Self> {
        let dirs = directories::BaseDirs::new().context("Cannot find home directory")?;
        let state_root = dirs.state_dir().unwrap_or(dirs.data_local_dir());
        // The app was called ytui before 0.2.0-alpha.12. Move its folders once so saved
        // settings, queues, history and caches keep working under the new name.
        for root in [dirs.config_dir(), state_root, dirs.cache_dir()] {
            migrate_renamed_dir(&root.join(LEGACY_NAME), &root.join(APP_NAME));
        }
        Ok(Self {
            config_override: None,
            xdg_state: dirs.state_dir().unwrap_or(dirs.data_local_dir()).into(),
            config: dirs.config_dir().join("vidcue"),
            state: dirs
                .state_dir()
                .unwrap_or(dirs.data_local_dir())
                .join("vidcue/rust"),
            cache: dirs.cache_dir().join("vidcue/rust"),
            // Python-era data (queue.db) is only read for migration and keeps its old path.
            legacy_data: dirs.data_local_dir().join(LEGACY_NAME),
            home: dirs.home_dir().into(),
        })
    }
    pub fn create(&self) -> Result<()> {
        for dir in [&self.config, &self.state, &self.cache] {
            fs::create_dir_all(dir)?;
        }
        Ok(())
    }
    pub fn config_file(&self) -> PathBuf {
        self.config_override
            .clone()
            .unwrap_or_else(|| self.config.join("rust.toml"))
    }
    pub fn load_config(&self) -> Result<Config> {
        let mut c: Config = if self.config_file().exists() {
            toml::from_str(&fs::read_to_string(self.config_file())?)
                .context("Invalid rust.toml; fix it or use --config with a different file")?
        } else {
            Config::default()
        };
        self.fill_directories(&mut c);
        c.validate()?;
        Ok(c)
    }
    pub fn fill_directories(&self, c: &mut Config) {
        if c.download.directory.as_os_str().is_empty() {
            c.download.directory = default_media_dir(&self.home.join("Videos"));
        }
        if c.download.audio_directory.as_os_str().is_empty() {
            c.download.audio_directory = default_media_dir(&self.home.join("Music"));
        }
    }
    pub fn save_config(&self, c: &Config) -> Result<()> {
        c.validate()?;
        atomic_write(&self.config_file(), toml::to_string_pretty(c)?.as_bytes())
    }
}
/// Current folder and binary name.
pub const APP_NAME: &str = "vidcue";
/// Name used by releases before the rename; still read for migration.
pub const LEGACY_NAME: &str = "ytui";

/// Default download folder: an existing `ytui` folder keeps receiving files, so a
/// library downloaded before the rename is not split across two folders.
fn default_media_dir(parent: &Path) -> PathBuf {
    let legacy = parent.join(LEGACY_NAME);
    if legacy.is_dir() {
        legacy
    } else {
        parent.join(APP_NAME)
    }
}

/// Move `old` to `new` when only `old` exists, then leave a symlink at `old` so an older
/// binary that is still running (or started later) sees the same files. Errors are
/// ignored: the app then starts with the new, empty folder and the old one is untouched.
pub fn migrate_renamed_dir(old: &Path, new: &Path) {
    let Ok(meta) = fs::symlink_metadata(old) else {
        return;
    };
    if fs::symlink_metadata(new).is_ok() {
        return;
    }
    if meta.file_type().is_symlink() {
        // A folder linked from elsewhere (for example dotfiles) stays where it is.
        #[cfg(unix)]
        if old.is_dir() {
            let _ = std::os::unix::fs::symlink(fs::canonicalize(old).unwrap_or(old.into()), new);
        }
        return;
    }
    if meta.is_dir() && fs::rename(old, new).is_ok() {
        #[cfg(unix)]
        let _ = std::os::unix::fs::symlink(new, old);
    }
}

/// Same-directory rename, private temporary file from creation, and durable flush.
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    write_atomic(path, data, true)
}
/// Atomically create a new file and refuse to replace even a racing creator.
pub fn atomic_write_new(path: &Path, data: &[u8]) -> Result<()> {
    write_atomic(path, data, false)
}
fn write_atomic(path: &Path, data: &[u8], replace: bool) -> Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(data)?;
    temp.as_file().sync_all()?;
    if replace {
        temp.persist(path).map_err(|e| e.error)?;
    } else {
        temp.persist_noclobber(path).map_err(|e| e.error)?;
    }
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}
pub fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Resolve tools for upstream libraries that require an explicit executable path.
pub fn executable(program: &str) -> Option<PathBuf> {
    let path = PathBuf::from(program);
    if path.components().count() > 1 {
        return path.is_file().then_some(path);
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(program))
            .find(|p| p.is_file())
    })
}

#[cfg(test)]
mod rename_tests {
    use super::*;

    #[test]
    fn renamed_folder_moves_once_and_keeps_old_path_working() {
        let t = tempfile::tempdir().unwrap();
        let (old, new) = (t.path().join("ytui"), t.path().join("vidcue"));
        fs::create_dir_all(old.join("rust")).unwrap();
        fs::write(old.join("rust.toml"), "theme = \"nord\"\n").unwrap();
        migrate_renamed_dir(&old, &new);
        assert_eq!(
            fs::read_to_string(new.join("rust.toml")).unwrap(),
            "theme = \"nord\"\n"
        );
        assert!(new.join("rust").is_dir());
        assert!(fs::symlink_metadata(&old).unwrap().file_type().is_symlink());
        assert!(old.join("rust.toml").is_file());
        // A second start with both present changes nothing.
        fs::write(new.join("rust.toml"), "theme = \"gruvbox\"\n").unwrap();
        migrate_renamed_dir(&old, &new);
        assert_eq!(
            fs::read_to_string(new.join("rust.toml")).unwrap(),
            "theme = \"gruvbox\"\n"
        );
    }

    #[test]
    fn existing_new_folder_and_missing_old_folder_are_left_alone() {
        let t = tempfile::tempdir().unwrap();
        let (old, new) = (t.path().join("ytui"), t.path().join("vidcue"));
        migrate_renamed_dir(&old, &new);
        assert!(!new.exists() && !old.exists());
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&new).unwrap();
        fs::write(old.join("keep"), "").unwrap();
        migrate_renamed_dir(&old, &new);
        assert!(old.join("keep").is_file() && !new.join("keep").exists());
    }

    #[cfg(unix)]
    #[test]
    fn linked_old_folder_is_linked_not_moved() {
        let t = tempfile::tempdir().unwrap();
        let target = t.path().join("dotfiles");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("rust.toml"), "").unwrap();
        let (old, new) = (t.path().join("ytui"), t.path().join("vidcue"));
        std::os::unix::fs::symlink(&target, &old).unwrap();
        migrate_renamed_dir(&old, &new);
        assert!(target.join("rust.toml").is_file());
        assert!(new.join("rust.toml").is_file());
    }

    #[test]
    fn downloads_keep_using_an_existing_ytui_folder() {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(default_media_dir(t.path()), t.path().join("vidcue"));
        fs::create_dir_all(t.path().join("ytui")).unwrap();
        assert_eq!(default_media_dir(t.path()), t.path().join("ytui"));
    }
}
