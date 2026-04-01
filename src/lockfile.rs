use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{RenkeiError, Result};
use crate::install_cache::PackageEntry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lockfile {
    #[serde(rename = "lockfileVersion")]
    pub lockfile_version: u32,
    pub packages: HashMap<String, LockfileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LockfileEntry {
    pub version: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<String>,
    pub integrity: String,
}

impl Lockfile {
    /// Load from disk, returning an empty lockfile if the file does not exist.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(content) => Ok(serde_json::from_str(&content)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self {
                lockfile_version: 1,
                packages: HashMap::new(),
            }),
            Err(e) => Err(e.into()),
        }
    }

    /// Load from disk, returning LockfileNotFound if the file does not exist.
    pub fn load_strict(path: &Path, scope_hint: &str) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(content) => Ok(serde_json::from_str(&content)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(RenkeiError::LockfileNotFound {
                    path: path.to_string_lossy().to_string(),
                    hint: scope_hint.to_string(),
                })
            }
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn upsert(&mut self, name: &str, entry: LockfileEntry) {
        self.packages.insert(name.to_string(), entry);
    }

    pub fn remove(&mut self, name: &str) {
        self.packages.remove(name);
    }
}

impl LockfileEntry {
    pub fn from_package_entry(entry: &PackageEntry) -> Self {
        Self {
            version: entry.version.clone(),
            source: entry.source_path.clone(),
            tag: entry.tag.clone(),
            resolved: entry.resolved.clone(),
            integrity: format!("sha256-{}", entry.integrity),
        }
    }
}

pub fn install_from_lockfile(
    _config: &crate::config::Config,
    _backend: &dyn crate::backend::Backend,
) -> Result<()> {
    todo!("Step 6: implement install-from-lockfile")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_lockfile_serialization_format() {
        let mut lockfile = Lockfile {
            lockfile_version: 1,
            packages: HashMap::new(),
        };
        lockfile.upsert(
            "@test/pkg",
            LockfileEntry {
                version: "1.0.0".to_string(),
                source: "git@github.com:user/repo".to_string(),
                tag: Some("v1.0.0".to_string()),
                resolved: Some("abc123".to_string()),
                integrity: "sha256-deadbeef".to_string(),
            },
        );

        let json = serde_json::to_string_pretty(&lockfile).unwrap();
        assert!(json.contains("\"lockfileVersion\": 1"));
        assert!(json.contains("sha256-deadbeef"));
        assert!(json.contains("\"tag\": \"v1.0.0\""));
        assert!(json.contains("\"resolved\": \"abc123\""));
    }

    #[test]
    fn test_lockfile_deserialization() {
        let json = r#"{
            "lockfileVersion": 1,
            "packages": {
                "@meryll/mr-review": {
                    "version": "1.2.0",
                    "source": "git@github.com:meryll/mr-review",
                    "tag": "v1.2.0",
                    "resolved": "abc123def",
                    "integrity": "sha256-xyz"
                }
            }
        }"#;
        let lockfile: Lockfile = serde_json::from_str(json).unwrap();
        assert_eq!(lockfile.lockfile_version, 1);
        let entry = &lockfile.packages["@meryll/mr-review"];
        assert_eq!(entry.version, "1.2.0");
        assert_eq!(entry.source, "git@github.com:meryll/mr-review");
        assert_eq!(entry.tag.as_deref(), Some("v1.2.0"));
        assert_eq!(entry.resolved.as_deref(), Some("abc123def"));
        assert_eq!(entry.integrity, "sha256-xyz");
    }

    #[test]
    fn test_lockfile_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rk.lock");

        let mut lockfile = Lockfile {
            lockfile_version: 1,
            packages: HashMap::new(),
        };
        lockfile.upsert(
            "@test/pkg",
            LockfileEntry {
                version: "2.0.0".to_string(),
                source: "/tmp/pkg".to_string(),
                tag: None,
                resolved: None,
                integrity: "sha256-abc".to_string(),
            },
        );
        lockfile.save(&path).unwrap();

        let loaded = Lockfile::load(&path).unwrap();
        assert_eq!(loaded.lockfile_version, 1);
        assert_eq!(loaded.packages.len(), 1);
        assert_eq!(loaded.packages["@test/pkg"].version, "2.0.0");
    }

    #[test]
    fn test_lockfile_upsert_adds_new() {
        let mut lockfile = Lockfile {
            lockfile_version: 1,
            packages: HashMap::new(),
        };
        lockfile.upsert(
            "@test/a",
            LockfileEntry {
                version: "1.0.0".to_string(),
                source: "/a".to_string(),
                tag: None,
                resolved: None,
                integrity: "sha256-a".to_string(),
            },
        );
        assert_eq!(lockfile.packages.len(), 1);
    }

    #[test]
    fn test_lockfile_upsert_replaces_existing() {
        let mut lockfile = Lockfile {
            lockfile_version: 1,
            packages: HashMap::new(),
        };
        lockfile.upsert(
            "@test/a",
            LockfileEntry {
                version: "1.0.0".to_string(),
                source: "/a".to_string(),
                tag: None,
                resolved: None,
                integrity: "sha256-a".to_string(),
            },
        );
        lockfile.upsert(
            "@test/a",
            LockfileEntry {
                version: "2.0.0".to_string(),
                source: "/a".to_string(),
                tag: None,
                resolved: None,
                integrity: "sha256-b".to_string(),
            },
        );
        assert_eq!(lockfile.packages.len(), 1);
        assert_eq!(lockfile.packages["@test/a"].version, "2.0.0");
    }

    #[test]
    fn test_lockfile_remove() {
        let mut lockfile = Lockfile {
            lockfile_version: 1,
            packages: HashMap::new(),
        };
        lockfile.upsert(
            "@test/a",
            LockfileEntry {
                version: "1.0.0".to_string(),
                source: "/a".to_string(),
                tag: None,
                resolved: None,
                integrity: "sha256-a".to_string(),
            },
        );
        lockfile.remove("@test/a");
        assert!(lockfile.packages.is_empty());
    }

    #[test]
    fn test_lockfile_load_returns_empty_when_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.lock");
        let lockfile = Lockfile::load(&path).unwrap();
        assert_eq!(lockfile.lockfile_version, 1);
        assert!(lockfile.packages.is_empty());
    }

    #[test]
    fn test_lockfile_load_strict_errors_when_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.lock");
        let result = Lockfile::load_strict(&path, "Use `rk install <source>` first.");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("No lockfile found"));
        assert!(err.contains("rk install <source>"));
    }

    #[test]
    fn test_from_package_entry_git() {
        let entry = PackageEntry {
            version: "1.0.0".to_string(),
            source: "git".to_string(),
            source_path: "git@github.com:user/repo".to_string(),
            integrity: "deadbeef".to_string(),
            archive_path: "/tmp/a.tar.gz".to_string(),
            deployed_artifacts: vec![],
            deployed_mcp_servers: vec![],
            resolved: Some("abc123".to_string()),
            tag: Some("v1.0.0".to_string()),
        };
        let lockfile_entry = LockfileEntry::from_package_entry(&entry);
        assert_eq!(lockfile_entry.version, "1.0.0");
        assert_eq!(lockfile_entry.source, "git@github.com:user/repo");
        assert_eq!(lockfile_entry.tag.as_deref(), Some("v1.0.0"));
        assert_eq!(lockfile_entry.resolved.as_deref(), Some("abc123"));
        assert_eq!(lockfile_entry.integrity, "sha256-deadbeef");
    }

    #[test]
    fn test_from_package_entry_local() {
        let entry = PackageEntry {
            version: "1.0.0".to_string(),
            source: "local".to_string(),
            source_path: "/tmp/pkg".to_string(),
            integrity: "aabbcc".to_string(),
            archive_path: "/tmp/a.tar.gz".to_string(),
            deployed_artifacts: vec![],
            deployed_mcp_servers: vec![],
            resolved: None,
            tag: None,
        };
        let lockfile_entry = LockfileEntry::from_package_entry(&entry);
        assert_eq!(lockfile_entry.source, "/tmp/pkg");
        assert!(lockfile_entry.tag.is_none());
        assert!(lockfile_entry.resolved.is_none());
        assert_eq!(lockfile_entry.integrity, "sha256-aabbcc");
    }

    #[test]
    fn test_lockfile_optional_fields_omitted() {
        let mut lockfile = Lockfile {
            lockfile_version: 1,
            packages: HashMap::new(),
        };
        lockfile.upsert(
            "@test/local",
            LockfileEntry {
                version: "1.0.0".to_string(),
                source: "/tmp/pkg".to_string(),
                tag: None,
                resolved: None,
                integrity: "sha256-abc".to_string(),
            },
        );
        let json = serde_json::to_string_pretty(&lockfile).unwrap();
        assert!(!json.contains("\"tag\""));
        assert!(!json.contains("\"resolved\""));
    }
}
