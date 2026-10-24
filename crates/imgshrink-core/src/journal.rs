use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JournalEntry {
    pub source: PathBuf,
    pub temp: PathBuf,
    pub started_unix: u64,
}

/// Registro mínimo de reemplazos in-place. Permite detectar operaciones que
/// quedaron a medias si el proceso muere entre la escritura y el reemplazo.
#[derive(Debug)]
pub struct Journal {
    path: PathBuf,
    lock: Mutex<()>,
}

impl Journal {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Mutex::new(()),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn read_unlocked(&self) -> Vec<JournalEntry> {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn write_unlocked(&self, entries: &[JournalEntry]) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(entries)
            .map_err(|error| io::Error::other(error.to_string()))?;
        let temp = self.path.with_extension("tmp");
        fs::write(&temp, text)?;
        fs::rename(&temp, &self.path)?;
        Ok(())
    }

    pub fn begin(&self, source: &Path, temp: &Path) -> io::Result<()> {
        let _guard = self.lock.lock().unwrap();
        let mut entries = self.read_unlocked();
        entries.retain(|entry| entry.source != source);
        entries.push(JournalEntry {
            source: source.to_path_buf(),
            temp: temp.to_path_buf(),
            started_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0),
        });
        self.write_unlocked(&entries)
    }

    pub fn finish(&self, source: &Path) -> io::Result<()> {
        let _guard = self.lock.lock().unwrap();
        let mut entries = self.read_unlocked();
        entries.retain(|entry| entry.source != source);
        self.write_unlocked(&entries)
    }

    pub fn pending(&self) -> Vec<JournalEntry> {
        let _guard = self.lock.lock().unwrap();
        self.read_unlocked()
    }
}
