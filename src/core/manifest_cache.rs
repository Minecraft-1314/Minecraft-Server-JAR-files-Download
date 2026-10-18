use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const DEFAULT_CACHE_MINUTES: u64 = 30;

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    source: String,
    timestamp: u64,
    manifest: serde_json::Value,
}

pub struct ManifestCache {
    path: PathBuf,
    duration: Duration,
}

impl ManifestCache {
    pub fn new(path: PathBuf, duration: Option<Duration>) -> Self {
        Self {
            path,
            duration: duration.unwrap_or(Duration::from_secs(DEFAULT_CACHE_MINUTES * 60)),
        }
    }

    pub fn read(&self, source: &str) -> Option<serde_json::Value> {
        let text = fs::read_to_string(&self.path).ok()?;
        let entry: CacheEntry = serde_json::from_str(&text).ok()?;
        if entry.source != source {
            return None;
        }
        let now = now_seconds();
        if now.saturating_sub(entry.timestamp) > self.duration.as_secs() {
            return None;
        }
        Some(entry.manifest)
    }

    pub fn write(&self, source: &str, manifest: &serde_json::Value) {
        let Some(directory) = self.path.parent() else {
            return;
        };
        if fs::create_dir_all(directory).is_err() {
            return;
        }
        let entry = CacheEntry {
            source: source.to_string(),
            timestamp: now_seconds(),
            manifest: manifest.clone(),
        };
        let Ok(text) = serde_json::to_string(&entry) else {
            return;
        };
        let temporary = self.path.with_extension("json.tmp");
        if fs::write(&temporary, text).is_err() {
            return;
        }
        let _ = fs::rename(&temporary, &self.path);
    }
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}
