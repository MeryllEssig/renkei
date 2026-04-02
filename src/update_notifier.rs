use std::fs;
use std::io::IsTerminal;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime};

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::self_update;

const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Serialize, Deserialize)]
struct VersionCache {
    latest_version: String,
    checked_at: u64,
}

impl VersionCache {
    fn is_stale(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(self.checked_at) > CHECK_INTERVAL.as_secs()
    }
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn load_cache() -> Option<VersionCache> {
    let path = self_update::cache_path();
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn save_cache(version: &Version) {
    let path = self_update::cache_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let cache = VersionCache {
        latest_version: version.to_string(),
        checked_at: now_epoch(),
    };
    if let Ok(json) = serde_json::to_string(&cache) {
        let _ = fs::write(path, json);
    }
}

/// A handle that can be queried after the main command completes.
pub struct UpdateCheck {
    receiver: mpsc::Receiver<Option<Version>>,
}

impl UpdateCheck {
    /// Print the update notification to stderr if a newer version was found.
    pub fn notify(self) {
        let latest = match self.receiver.recv_timeout(Duration::from_millis(500)) {
            Ok(Some(v)) => v,
            _ => return,
        };

        let current = self_update::current_version();
        if latest <= current {
            return;
        }

        let line1 = format!("New version available: {current} → {latest}");
        let line2 = "Run `rk self-update` to update";
        let width = line1.len().max(line2.len()) + 4;
        eprintln!();
        eprintln!("  ╭{}╮", "─".repeat(width));
        eprintln!("  │  {:<w$}  │", line1, w = width - 4);
        eprintln!("  │  {:<w$}  │", line2, w = width - 4);
        eprintln!("  ╰{}╯", "─".repeat(width));
        eprintln!();
    }
}

/// Spawn a background version check. Returns a handle to query later.
/// Returns None if we should skip (not a TTY, cache is fresh).
pub fn spawn_check() -> Option<UpdateCheck> {
    if !std::io::stderr().is_terminal() {
        return None;
    }

    // Check cache first — if fresh and no update, skip entirely
    if let Some(cache) = load_cache() {
        if !cache.is_stale() {
            // Cache is fresh — check if cached version is newer
            if let Ok(cached_ver) = Version::parse(&cache.latest_version) {
                let current = self_update::current_version();
                if cached_ver > current {
                    // We know there's an update — return it immediately
                    let (tx, rx) = mpsc::channel();
                    let _ = tx.send(Some(cached_ver));
                    return Some(UpdateCheck { receiver: rx });
                }
            }
            // Cache is fresh and no update needed
            return None;
        }
    }

    // Cache is stale or missing — spawn background check
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = self_update::fetch_latest_version().ok();
        if let Some(ref v) = result {
            save_cache(v);
        }
        let _ = tx.send(result);
    });

    Some(UpdateCheck { receiver: rx })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_is_stale_when_old() {
        let cache = VersionCache {
            latest_version: "1.0.0".to_string(),
            checked_at: 0, // epoch = very old
        };
        assert!(cache.is_stale());
    }

    #[test]
    fn test_cache_is_fresh_when_recent() {
        let cache = VersionCache {
            latest_version: "1.0.0".to_string(),
            checked_at: now_epoch(),
        };
        assert!(!cache.is_stale());
    }

    #[test]
    fn test_cache_roundtrip() {
        let v = Version::new(2, 1, 0);
        let cache = VersionCache {
            latest_version: v.to_string(),
            checked_at: now_epoch(),
        };
        let json = serde_json::to_string(&cache).unwrap();
        let parsed: VersionCache = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.latest_version, "2.1.0");
        assert!(!parsed.is_stale());
    }
}
