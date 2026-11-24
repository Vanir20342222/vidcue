//! Secret storage the platform seals, and plain JSON state files (PROTOCOL.md §7.1).
use anyhow::{Context, Result, ensure};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::HashMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

/// Secret names.
pub const SESSION: &str = "session";
pub const E2E_KEY: &str = "e2e_key";
/// State files are library data, never secrets; this bounds what is read back.
const MAX_STATE_BYTES: u64 = 64 * 1024 * 1024;

/// Sealed storage for the device token and the account key. The platform decides how
/// values are protected (Android: Keystore AES-GCM).
pub trait SecretStore: Send + Sync {
    fn load(&self, name: &str) -> Result<Option<String>>;
    fn save(&self, name: &str, value: &str) -> Result<()>;
    fn delete(&self, name: &str) -> Result<()>;
}

/// In-memory secrets for tests.
#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<String, String>>);

impl SecretStore for MemoryStore {
    fn load(&self, name: &str) -> Result<Option<String>> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(name)
            .cloned())
    }
    fn save(&self, name: &str, value: &str) -> Result<()> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(name.into(), value.into());
        Ok(())
    }
    fn delete(&self, name: &str) -> Result<()> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(name);
        Ok(())
    }
}

/// Reads a JSON file; a missing file is `None`.
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_STATE_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_STATE_BYTES,
        "Sync state file exceeds limit"
    );
    Ok(Some(serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "Sync state file {} is unreadable",
            path.file_name().unwrap_or_default().to_string_lossy()
        )
    })?))
}

/// Writes JSON atomically: a temporary file in the same directory, flushed, then renamed.
pub fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    write_atomic(path, &serde_json::to_vec(value)?)
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().context("State path has no directory")?;
    std::fs::create_dir_all(dir)?;
    let mut temp = tempfile_in(dir)?;
    temp.1.write_all(bytes)?;
    temp.1.sync_all()?;
    drop(temp.1);
    std::fs::rename(&temp.0, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp.0);
    })?;
    if let Ok(dir) = std::fs::File::open(dir) {
        let _ = dir.sync_all();
    }
    Ok(())
}

fn tempfile_in(dir: &Path) -> Result<(PathBuf, std::fs::File)> {
    for _ in 0..8 {
        let path = dir.join(format!(".tmp-{}", uuid::Uuid::new_v4().simple()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => return Ok((path, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    anyhow::bail!("Could not create a temporary state file")
}

/// Removes a file; a missing file is fine.
pub fn remove_file(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_files_round_trip_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/state.json");
        assert!(read_json::<serde_json::Value>(&path).unwrap().is_none());
        write_json(&path, &serde_json::json!({"cursor": 5})).unwrap();
        assert_eq!(
            read_json::<serde_json::Value>(&path).unwrap().unwrap()["cursor"],
            5
        );
        // No temporary files are left behind.
        assert_eq!(
            std::fs::read_dir(dir.path().join("sub")).unwrap().count(),
            1
        );
        std::fs::write(&path, "{").unwrap();
        assert!(read_json::<serde_json::Value>(&path).is_err());
        remove_file(&path).unwrap();
        remove_file(&path).unwrap();
        let store = MemoryStore::default();
        store.save(SESSION, "x").unwrap();
        assert_eq!(store.load(SESSION).unwrap().as_deref(), Some("x"));
        store.delete(SESSION).unwrap();
        assert!(store.load(SESSION).unwrap().is_none());
    }
}
