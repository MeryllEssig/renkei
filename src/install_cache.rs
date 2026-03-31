use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::artifact::ArtifactKind;
use crate::config::Config;
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallCache {
    pub version: u32,
    pub packages: HashMap<String, PackageEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageEntry {
    pub version: String,
    pub source: String,
    pub source_path: String,
    pub integrity: String,
    pub archive_path: String,
    pub deployed_artifacts: Vec<DeployedArtifactEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployedArtifactEntry {
    pub artifact_type: ArtifactKind,
    pub name: String,
    pub deployed_path: String,
}

impl InstallCache {
    pub fn load(config: &Config) -> Result<Self> {
        let path = config.install_cache_path();
        if !path.exists() {
            return Ok(Self {
                version: 1,
                packages: HashMap::new(),
            });
        }
        let content = std::fs::read_to_string(&path)?;
        let cache: InstallCache = serde_json::from_str(&content)?;
        Ok(cache)
    }

    pub fn save(&self, config: &Config) -> Result<()> {
        let path = config.install_cache_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn upsert_package(&mut self, full_name: &str, entry: PackageEntry) {
        self.packages.insert(full_name.to_string(), entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_nonexistent() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());
        let cache = InstallCache::load(&config).unwrap();
        assert_eq!(cache.version, 1);
        assert!(cache.packages.is_empty());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());

        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package(
            "@test/sample",
            PackageEntry {
                version: "1.0.0".to_string(),
                source: "local".to_string(),
                source_path: "/tmp/pkg".to_string(),
                integrity: "abc123".to_string(),
                archive_path: "/tmp/archive.tar.gz".to_string(),
                deployed_artifacts: vec![DeployedArtifactEntry {
                    artifact_type: ArtifactKind::Skill,
                    name: "review".to_string(),
                    deployed_path: "/home/.claude/skills/renkei-review/SKILL.md".to_string(),
                }],
            },
        );
        cache.save(&config).unwrap();

        let loaded = InstallCache::load(&config).unwrap();
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.packages.len(), 1);
        let entry = &loaded.packages["@test/sample"];
        assert_eq!(entry.version, "1.0.0");
        assert_eq!(entry.deployed_artifacts.len(), 1);
        assert_eq!(entry.deployed_artifacts[0].name, "review");
    }

    #[test]
    fn test_upsert_package() {
        let mut cache = InstallCache {
            version: 1,
            packages: HashMap::new(),
        };

        cache.upsert_package(
            "@test/pkg",
            PackageEntry {
                version: "1.0.0".to_string(),
                source: "local".to_string(),
                source_path: "/a".to_string(),
                integrity: "aaa".to_string(),
                archive_path: "/a.tar.gz".to_string(),
                deployed_artifacts: vec![],
            },
        );
        assert_eq!(cache.packages["@test/pkg"].version, "1.0.0");

        cache.upsert_package(
            "@test/pkg",
            PackageEntry {
                version: "2.0.0".to_string(),
                source: "local".to_string(),
                source_path: "/b".to_string(),
                integrity: "bbb".to_string(),
                archive_path: "/b.tar.gz".to_string(),
                deployed_artifacts: vec![],
            },
        );
        assert_eq!(cache.packages.len(), 1);
        assert_eq!(cache.packages["@test/pkg"].version, "2.0.0");
    }
}
