use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::artifact::ArtifactKind;
use crate::config::Config;
use crate::error::Result;
use crate::hook::DeployedHookEntry;

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deployed_mcp_servers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployedArtifactEntry {
    pub artifact_type: ArtifactKind,
    pub name: String,
    pub deployed_path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deployed_hooks: Vec<DeployedHookEntry>,
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
                    deployed_hooks: vec![],
                }],
                deployed_mcp_servers: vec![],
                resolved: None,
                tag: None,
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
                deployed_mcp_servers: vec![],
                resolved: None,
                tag: None,
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
                deployed_mcp_servers: vec![],
                resolved: None,
                tag: None,
            },
        );
        assert_eq!(cache.packages.len(), 1);
        assert_eq!(cache.packages["@test/pkg"].version, "2.0.0");
    }

    #[test]
    fn test_save_and_load_with_hooks() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());

        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package(
            "@test/hook-pkg",
            PackageEntry {
                version: "1.0.0".to_string(),
                source: "local".to_string(),
                source_path: "/tmp/pkg".to_string(),
                integrity: "abc".to_string(),
                archive_path: "/tmp/a.tar.gz".to_string(),
                deployed_artifacts: vec![DeployedArtifactEntry {
                    artifact_type: ArtifactKind::Hook,
                    name: "lint".to_string(),
                    deployed_path: "/home/.claude/settings.json".to_string(),
                    deployed_hooks: vec![DeployedHookEntry {
                        event: "PreToolUse".to_string(),
                        matcher: Some("bash".to_string()),
                        command: "lint.sh".to_string(),
                    }],
                }],
                deployed_mcp_servers: vec![],
                resolved: None,
                tag: None,
            },
        );
        cache.save(&config).unwrap();

        let loaded = InstallCache::load(&config).unwrap();
        let entry = &loaded.packages["@test/hook-pkg"];
        assert_eq!(
            entry.deployed_artifacts[0].artifact_type,
            ArtifactKind::Hook
        );
        assert_eq!(entry.deployed_artifacts[0].deployed_hooks.len(), 1);
        assert_eq!(
            entry.deployed_artifacts[0].deployed_hooks[0].event,
            "PreToolUse"
        );
        assert_eq!(
            entry.deployed_artifacts[0].deployed_hooks[0].command,
            "lint.sh"
        );
    }

    #[test]
    fn test_load_legacy_cache_without_hooks_field() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());

        // Write a cache entry without deployed_hooks field (legacy format)
        let legacy_json = r#"{
            "version": 1,
            "packages": {
                "@test/legacy": {
                    "version": "1.0.0",
                    "source": "local",
                    "source_path": "/tmp",
                    "integrity": "abc",
                    "archive_path": "/tmp/a.tar.gz",
                    "deployed_artifacts": [
                        {"artifact_type": "skill", "name": "review", "deployed_path": "/p"}
                    ]
                }
            }
        }"#;
        let cache_path = config.install_cache_path();
        std::fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
        std::fs::write(&cache_path, legacy_json).unwrap();

        let loaded = InstallCache::load(&config).unwrap();
        let entry = &loaded.packages["@test/legacy"];
        assert_eq!(entry.deployed_artifacts[0].deployed_hooks.len(), 0);
        assert!(entry.deployed_mcp_servers.is_empty());
    }

    #[test]
    fn test_save_and_load_with_mcp_servers() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());

        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package(
            "@test/mcp-pkg",
            PackageEntry {
                version: "1.0.0".to_string(),
                source: "local".to_string(),
                source_path: "/tmp/pkg".to_string(),
                integrity: "abc".to_string(),
                archive_path: "/tmp/a.tar.gz".to_string(),
                deployed_artifacts: vec![],
                deployed_mcp_servers: vec!["test-server".to_string(), "api-server".to_string()],
                resolved: None,
                tag: None,
            },
        );
        cache.save(&config).unwrap();

        let loaded = InstallCache::load(&config).unwrap();
        let entry = &loaded.packages["@test/mcp-pkg"];
        assert_eq!(entry.deployed_mcp_servers.len(), 2);
        assert!(entry
            .deployed_mcp_servers
            .contains(&"test-server".to_string()));
        assert!(entry
            .deployed_mcp_servers
            .contains(&"api-server".to_string()));
    }

    #[test]
    fn test_project_scope_save_and_load_roundtrip() {
        let home = tempdir().unwrap();
        let config = Config::for_project(
            home.path().to_path_buf(),
            std::path::PathBuf::from("/Users/test/Projects/foo"),
        );

        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package(
            "@test/project-pkg",
            PackageEntry {
                version: "1.0.0".to_string(),
                source: "local".to_string(),
                source_path: "/tmp/pkg".to_string(),
                integrity: "abc".to_string(),
                archive_path: "/tmp/a.tar.gz".to_string(),
                deployed_artifacts: vec![],
                deployed_mcp_servers: vec![],
                resolved: None,
                tag: None,
            },
        );
        cache.save(&config).unwrap();

        // Verify file is at the project-specific path
        let expected_path = home
            .path()
            .join(".renkei/projects/Users-test-Projects-foo/install-cache.json");
        assert!(expected_path.exists());

        // Roundtrip
        let loaded = InstallCache::load(&config).unwrap();
        assert_eq!(loaded.packages.len(), 1);
        assert_eq!(loaded.packages["@test/project-pkg"].version, "1.0.0");
    }

    #[test]
    fn test_project_and_global_caches_independent() {
        let home = tempdir().unwrap();
        let global_config = Config::with_home_dir(home.path().to_path_buf());
        let project_config = Config::for_project(
            home.path().to_path_buf(),
            std::path::PathBuf::from("/Users/test/myproject"),
        );

        // Save to global cache
        let mut global_cache = InstallCache::load(&global_config).unwrap();
        global_cache.upsert_package(
            "@test/global-pkg",
            PackageEntry {
                version: "1.0.0".to_string(),
                source: "local".to_string(),
                source_path: "/tmp/g".to_string(),
                integrity: "aaa".to_string(),
                archive_path: "/tmp/g.tar.gz".to_string(),
                deployed_artifacts: vec![],
                deployed_mcp_servers: vec![],
                resolved: None,
                tag: None,
            },
        );
        global_cache.save(&global_config).unwrap();

        // Save to project cache
        let mut project_cache = InstallCache::load(&project_config).unwrap();
        project_cache.upsert_package(
            "@test/project-pkg",
            PackageEntry {
                version: "2.0.0".to_string(),
                source: "local".to_string(),
                source_path: "/tmp/p".to_string(),
                integrity: "bbb".to_string(),
                archive_path: "/tmp/p.tar.gz".to_string(),
                deployed_artifacts: vec![],
                deployed_mcp_servers: vec![],
                resolved: None,
                tag: None,
            },
        );
        project_cache.save(&project_config).unwrap();

        // Load each independently — they don't contaminate each other
        let loaded_global = InstallCache::load(&global_config).unwrap();
        assert_eq!(loaded_global.packages.len(), 1);
        assert!(loaded_global.packages.contains_key("@test/global-pkg"));
        assert!(!loaded_global.packages.contains_key("@test/project-pkg"));

        let loaded_project = InstallCache::load(&project_config).unwrap();
        assert_eq!(loaded_project.packages.len(), 1);
        assert!(loaded_project.packages.contains_key("@test/project-pkg"));
        assert!(!loaded_project.packages.contains_key("@test/global-pkg"));
    }

    #[test]
    fn test_save_and_load_with_git_fields() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());

        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package(
            "@test/git-pkg",
            PackageEntry {
                version: "1.0.0".to_string(),
                source: "git".to_string(),
                source_path: "git@github.com:user/repo".to_string(),
                integrity: "abc".to_string(),
                archive_path: "/tmp/a.tar.gz".to_string(),
                deployed_artifacts: vec![],
                deployed_mcp_servers: vec![],
                resolved: Some("abcdef1234567890abcdef1234567890abcdef12".to_string()),
                tag: Some("v1.0.0".to_string()),
            },
        );
        cache.save(&config).unwrap();

        let loaded = InstallCache::load(&config).unwrap();
        let entry = &loaded.packages["@test/git-pkg"];
        assert_eq!(entry.source, "git");
        assert_eq!(
            entry.resolved.as_deref(),
            Some("abcdef1234567890abcdef1234567890abcdef12")
        );
        assert_eq!(entry.tag.as_deref(), Some("v1.0.0"));
    }

    #[test]
    fn test_none_fields_omitted_from_json() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());

        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package(
            "@test/local-pkg",
            PackageEntry {
                version: "1.0.0".to_string(),
                source: "local".to_string(),
                source_path: "/tmp/pkg".to_string(),
                integrity: "abc".to_string(),
                archive_path: "/tmp/a.tar.gz".to_string(),
                deployed_artifacts: vec![],
                deployed_mcp_servers: vec![],
                resolved: None,
                tag: None,
            },
        );
        cache.save(&config).unwrap();

        let raw = std::fs::read_to_string(config.install_cache_path()).unwrap();
        assert!(!raw.contains("\"resolved\""));
        assert!(!raw.contains("\"tag\""));
    }

    #[test]
    fn test_load_legacy_cache_without_git_fields() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());

        let legacy_json = r#"{
            "version": 1,
            "packages": {
                "@test/old-pkg": {
                    "version": "1.0.0",
                    "source": "local",
                    "source_path": "/tmp",
                    "integrity": "abc",
                    "archive_path": "/tmp/a.tar.gz",
                    "deployed_artifacts": []
                }
            }
        }"#;
        let cache_path = config.install_cache_path();
        std::fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
        std::fs::write(&cache_path, legacy_json).unwrap();

        let loaded = InstallCache::load(&config).unwrap();
        let entry = &loaded.packages["@test/old-pkg"];
        assert!(entry.resolved.is_none());
        assert!(entry.tag.is_none());
    }
}
