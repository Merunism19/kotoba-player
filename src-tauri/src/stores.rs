//! The registry of every file the app stores (one place, explicit backup decision), with atomic JSON reads/writes.
//!
//! Rule (DESIGN §8, WORKFLOW gate 5): no module opens its own settings/storage file; add a `Store`
//! variant here instead, with a reason for its backup decision.

use serde::{Serialize, de::DeserializeOwned};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Whether a store belongs in a future backup/export (read by the export feature; tests check every store has one).
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backup {
    /// User data worth keeping (positions, choices).
    Include,
}

/// Every persistent store of the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Store {
    /// Per-file state: resume position, subtitle offset, chosen tracks, last opened.
    Library,
    /// User settings (Anki deck/tags…). Secrets are never stored here (OS keyring).
    Settings,
}

impl Store {
    /// All stores, for backup/export and tests.
    #[cfg_attr(not(test), allow(dead_code))]
    pub const ALL: &'static [Store] = &[Store::Library, Store::Settings];

    /// File name inside the app data directory.
    pub fn file_name(self) -> &'static str {
        match self {
            Store::Library => "library.json",
            Store::Settings => "settings.json",
        }
    }

    /// Backup decision (and why).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn backup(self) -> Backup {
        match self {
            // Losing it loses every resume point and per-episode subtitle sync.
            Store::Library => Backup::Include,
            // The user's choices; small and worth keeping.
            Store::Settings => Backup::Include,
        }
    }

    /// Full path of the store in `data_dir`.
    pub fn path(self, data_dir: &Path) -> PathBuf {
        data_dir.join(self.file_name())
    }
}

/// Move an unreadable store aside with a timestamp (never overwriting an earlier one), so starting
/// fresh can't destroy the user's data.
fn set_aside(path: &Path, why: &str) {
    let aside = path.with_extension(format!("json.bad-{}", crate::library::now_ms()));
    log_warn(&format!(
        "{} is unreadable ({why}); starting fresh and keeping it as {}",
        path.display(),
        aside.display()
    ));
    let _ = std::fs::rename(path, aside);
}

/// Read a store. A missing file gives the default; a file that can't be read (after a few retries,
/// e.g. a sharing violation) or parsed is moved aside first, so the next save can't overwrite it.
pub fn load<T: DeserializeOwned + Default>(data_dir: &Path, store: Store) -> T {
    let path = store.path(data_dir);
    let mut last_err = None;
    for attempt in 0..3 {
        match std::fs::read(&path) {
            Ok(bytes) => {
                return match serde_json::from_slice(&bytes) {
                    Ok(v) => v,
                    Err(e) => {
                        set_aside(&path, &e.to_string());
                        T::default()
                    }
                };
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return T::default(),
            Err(e) => {
                last_err = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(100 * (attempt + 1)));
            }
        }
    }
    set_aside(&path, &last_err.map(|e| e.to_string()).unwrap_or_default());
    T::default()
}

/// One writer per store at a time (the temp file name is shared).
static WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Write a store atomically: temp file in the same directory, flush to disk, then rename over the old one.
pub fn save<T: Serialize>(data_dir: &Path, store: Store, value: &T) -> std::io::Result<()> {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    std::fs::create_dir_all(data_dir)?;
    let path = store.path(data_dir);
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?;
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&json)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, &path)
}

fn log_warn(msg: &str) {
    eprintln!("[kotoba] {msg}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kp-stores-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn roundtrip_and_missing_file() {
        let d = tmpdir("rt");
        let empty: BTreeMap<String, u32> = load(&d, Store::Library);
        assert!(empty.is_empty());
        let mut m = BTreeMap::new();
        m.insert("a".to_string(), 1u32);
        save(&d, Store::Library, &m).unwrap();
        let back: BTreeMap<String, u32> = load(&d, Store::Library);
        assert_eq!(back, m);
        assert!(!Store::Library.path(&d).with_extension("json.tmp").exists());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn corrupt_file_is_set_aside() {
        let d = tmpdir("bad");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(Store::Library.path(&d), b"{not json").unwrap();
        let v: BTreeMap<String, u32> = load(&d, Store::Library);
        assert!(v.is_empty());
        std::thread::sleep(std::time::Duration::from_millis(5));
        // A second corrupt file is kept too (timestamped names never overwrite each other).
        std::fs::write(Store::Library.path(&d), b"{also bad").unwrap();
        let _: BTreeMap<String, u32> = load(&d, Store::Library);
        let aside = std::fs::read_dir(&d)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("json.bad-"))
            .count();
        assert_eq!(aside, 2);
        assert!(!Store::Library.path(&d).exists());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn every_store_has_a_backup_decision_and_unique_name() {
        let mut names: Vec<_> = Store::ALL.iter().map(|s| s.file_name()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), Store::ALL.len());
        for s in Store::ALL {
            let _ = s.backup();
        }
    }
}
