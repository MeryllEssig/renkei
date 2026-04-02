use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::artifact::ArtifactKind;
use crate::config::Config;
use crate::error::Result;
use crate::hook::DeployedHookEntry;

const CURRENT_VERSION: u32 = 2;

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
    pub deployed: HashMap<String, BackendDeployment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BackendDeployment {
    pub artifacts: Vec<DeployedArtifactEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp_servers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployedArtifactEntry {
    pub artifact_type: ArtifactKind,
    pub name: String,
    pub deployed_path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deployed_hooks: Vec<DeployedHookEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_name: Option<String>,
}

// --- v1 structs for migration ---

#[derive(Deserialize)]
struct V1Cache {
    #[allow(dead_code)]
    version: u32,
    packages: HashMap<String, V1PackageEntry>,
}

#[derive(Deserialize)]
struct V1PackageEntry {
    version: String,
    source: String,
    source_path: String,
    integrity: String,
    archive_path: String,
    deployed_artifacts: Vec<DeployedArtifactEntry>,
    #[serde(default)]
    deployed_mcp_servers: Vec<String>,
    #[serde(default)]
    resolved: Option<String>,
    #[serde(default)]
    tag: Option<String>,
}

impl PackageEntry {
    /// Iterate all deployed artifacts across all backends.
    pub fn all_artifacts(&self) -> impl Iterator<Item = &DeployedArtifactEntry> {
        self.deployed.values().flat_map(|d| d.artifacts.iter())
    }

    /// Collect all MCP server names across all backends.
    pub fn all_mcp_servers(&self) -> Vec<&str> {
        self.deployed
            .values()
            .flat_map(|d| d.mcp_servers.iter().map(|s| s.as_str()))
            .collect()
    }
}

impl InstallCache {
    pub fn load(config: &Config) -> Result<Self> {
        let path = config.install_cache_path();
        if !path.exists() {
            return Ok(Self {
                version: CURRENT_VERSION,
                packages: HashMap::new(),
            });
        }
        let content = std::fs::read_to_string(&path)?;

        // Peek at version to decide how to deserialize
        let raw: serde_json::Value = serde_json::from_str(&content)?;
        let version = raw
            .get("version")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;

        if version >= CURRENT_VERSION {
            let cache: InstallCache = serde_json::from_value(raw)?;
            return Ok(cache);
        }

        // v1 → v2 migration
        let v1: V1Cache = serde_json::from_value(raw)?;
        let mut cache = Self::migrate_v1(v1);

        // Save migrated cache
        cache.save(config)?;
        Ok(cache)
    }

    fn migrate_v1(v1: V1Cache) -> Self {
        let mut packages = HashMap::new();
        for (name, v1_entry) in v1.packages {
            let mut deployed = HashMap::new();
            let deployment = BackendDeployment {
                artifacts: v1_entry.deployed_artifacts,
                mcp_servers: v1_entry.deployed_mcp_servers,
            };
            deployed.insert("claude".to_string(), deployment);

            packages.insert(
                name,
                PackageEntry {
                    version: v1_entry.version,
                    source: v1_entry.source,
                    source_path: v1_entry.source_path,
                    integrity: v1_entry.integrity,
                    archive_path: v1_entry.archive_path,
                    deployed,
                    resolved: v1_entry.resolved,
                    tag: v1_entry.tag,
                },
            );
        }
        InstallCache {
            version: CURRENT_VERSION,
            packages,
        }
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

    fn make_v2_entry(backend: &str, artifacts: Vec<DeployedArtifactEntry>) -> PackageEntry {
        let mut deployed = HashMap::new();
        deployed.insert(
            backend.to_string(),
            BackendDeployment {
                artifacts,
                mcp_servers: vec![],
            },
        );
        PackageEntry {
            version: "1.0.0".to_string(),
            source: "local".to_string(),
            source_path: "/tmp/pkg".to_string(),
            integrity: "abc123".to_string(),
            archive_path: "/tmp/archive.tar.gz".to_string(),
            deployed,
            resolved: None,
            tag: None,
        }
    }

    fn make_artifact(kind: ArtifactKind, name: &str, path: &str) -> DeployedArtifactEntry {
        DeployedArtifactEntry {
            artifact_type: kind,
            name: name.to_string(),
            deployed_path: path.to_string(),
            deployed_hooks: vec![],
            original_name: None,
        }
    }

    #[test]
    fn test_load_nonexistent_creates_v2() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());
        let cache = InstallCache::load(&config).unwrap();
        assert_eq!(cache.version, 2);
        assert!(cache.packages.is_empty());
    }

    #[test]
    fn test_v2_save_and_load_roundtrip() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());

        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package(
            "@test/sample",
            make_v2_entry(
                "claude",
                vec![make_artifact(
                    ArtifactKind::Skill,
                    "review",
                    "/home/.claude/skills/renkei-review/SKILL.md",
                )],
            ),
        );
        cache.save(&config).unwrap();

        let loaded = InstallCache::load(&config).unwrap();
        assert_eq!(loaded.version, 2);
        assert_eq!(loaded.packages.len(), 1);
        let entry = &loaded.packages["@test/sample"];
        assert_eq!(entry.version, "1.0.0");
        let claude_deploy = &entry.deployed["claude"];
        assert_eq!(claude_deploy.artifacts.len(), 1);
        assert_eq!(claude_deploy.artifacts[0].name, "review");
    }

    #[test]
    fn test_v2_per_backend_grouping() {
        let mut deployed = HashMap::new();
        deployed.insert(
            "claude".to_string(),
            BackendDeployment {
                artifacts: vec![make_artifact(
                    ArtifactKind::Skill,
                    "review",
                    "/claude/path",
                )],
                mcp_servers: vec!["srv-a".to_string()],
            },
        );
        deployed.insert(
            "agents".to_string(),
            BackendDeployment {
                artifacts: vec![make_artifact(
                    ArtifactKind::Skill,
                    "review",
                    "/agents/path",
                )],
                mcp_servers: vec![],
            },
        );

        let entry = PackageEntry {
            version: "1.0.0".to_string(),
            source: "local".to_string(),
            source_path: "/tmp".to_string(),
            integrity: "abc".to_string(),
            archive_path: "/tmp/a.tar.gz".to_string(),
            deployed,
            resolved: None,
            tag: None,
        };

        // all_artifacts flattens across backends
        let all: Vec<_> = entry.all_artifacts().collect();
        assert_eq!(all.len(), 2);

        // all_mcp_servers flattens
        let mcps = entry.all_mcp_servers();
        assert_eq!(mcps, vec!["srv-a"]);
    }

    #[test]
    fn test_v1_to_v2_migration_wraps_under_claude() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());

        let v1_json = r#"{
            "version": 1,
            "packages": {
                "@test/migrated": {
                    "version": "1.0.0",
                    "source": "local",
                    "source_path": "/tmp/pkg",
                    "integrity": "abc",
                    "archive_path": "/tmp/a.tar.gz",
                    "deployed_artifacts": [
                        {"artifact_type": "skill", "name": "review", "deployed_path": "/p/SKILL.md"}
                    ],
                    "deployed_mcp_servers": ["test-server"]
                }
            }
        }"#;
        let cache_path = config.install_cache_path();
        std::fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
        std::fs::write(&cache_path, v1_json).unwrap();

        let loaded = InstallCache::load(&config).unwrap();
        assert_eq!(loaded.version, 2);

        let entry = &loaded.packages["@test/migrated"];
        assert!(entry.deployed.contains_key("claude"));
        assert_eq!(entry.deployed.len(), 1);

        let claude = &entry.deployed["claude"];
        assert_eq!(claude.artifacts.len(), 1);
        assert_eq!(claude.artifacts[0].name, "review");
        assert_eq!(claude.mcp_servers, vec!["test-server"]);
    }

    #[test]
    fn test_v1_migration_preserves_all_artifacts() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());

        let v1_json = r#"{
            "version": 1,
            "packages": {
                "@test/multi": {
                    "version": "2.0.0",
                    "source": "git",
                    "source_path": "git@github.com:user/repo",
                    "integrity": "def",
                    "archive_path": "/tmp/b.tar.gz",
                    "deployed_artifacts": [
                        {"artifact_type": "skill", "name": "review", "deployed_path": "/p1"},
                        {"artifact_type": "agent", "name": "deploy", "deployed_path": "/p2"},
                        {"artifact_type": "hook", "name": "lint", "deployed_path": "/p3",
                         "deployed_hooks": [{"event": "PreToolUse", "matcher": "bash", "command": "lint.sh"}]}
                    ],
                    "resolved": "abc123",
                    "tag": "v2.0.0"
                }
            }
        }"#;
        let cache_path = config.install_cache_path();
        std::fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
        std::fs::write(&cache_path, v1_json).unwrap();

        let loaded = InstallCache::load(&config).unwrap();
        let entry = &loaded.packages["@test/multi"];
        let claude = &entry.deployed["claude"];
        assert_eq!(claude.artifacts.len(), 3);
        assert_eq!(entry.resolved.as_deref(), Some("abc123"));
        assert_eq!(entry.tag.as_deref(), Some("v2.0.0"));
    }

    #[test]
    fn test_v1_migration_preserves_mcp_servers() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());

        let v1_json = r#"{
            "version": 1,
            "packages": {
                "@test/mcp": {
                    "version": "1.0.0",
                    "source": "local",
                    "source_path": "/tmp",
                    "integrity": "abc",
                    "archive_path": "/tmp/a.tar.gz",
                    "deployed_artifacts": [],
                    "deployed_mcp_servers": ["srv-a", "srv-b"]
                }
            }
        }"#;
        let cache_path = config.install_cache_path();
        std::fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
        std::fs::write(&cache_path, v1_json).unwrap();

        let loaded = InstallCache::load(&config).unwrap();
        let claude = &loaded.packages["@test/mcp"].deployed["claude"];
        assert_eq!(claude.mcp_servers, vec!["srv-a", "srv-b"]);
    }

    #[test]
    fn test_v1_migration_saves_as_v2() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());

        let v1_json = r#"{
            "version": 1,
            "packages": {
                "@test/pkg": {
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
        std::fs::write(&cache_path, v1_json).unwrap();

        // Load triggers migration + save
        InstallCache::load(&config).unwrap();

        // Re-read raw JSON to verify v2 format on disk
        let raw: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&cache_path).unwrap()).unwrap();
        assert_eq!(raw["version"], 2);
        assert!(raw["packages"]["@test/pkg"]["deployed"].is_object());
        assert!(raw["packages"]["@test/pkg"]
            .get("deployed_artifacts")
            .is_none());
    }

    #[test]
    fn test_upsert_package() {
        let mut cache = InstallCache {
            version: CURRENT_VERSION,
            packages: HashMap::new(),
        };

        cache.upsert_package("@test/pkg", make_v2_entry("claude", vec![]));
        assert_eq!(cache.packages["@test/pkg"].version, "1.0.0");

        let mut entry = make_v2_entry("claude", vec![]);
        entry.version = "2.0.0".to_string();
        cache.upsert_package("@test/pkg", entry);
        assert_eq!(cache.packages.len(), 1);
        assert_eq!(cache.packages["@test/pkg"].version, "2.0.0");
    }

    #[test]
    fn test_all_artifacts_empty() {
        let entry = make_v2_entry("claude", vec![]);
        assert_eq!(entry.all_artifacts().count(), 0);
    }

    #[test]
    fn test_all_mcp_servers_empty() {
        let entry = make_v2_entry("claude", vec![]);
        assert!(entry.all_mcp_servers().is_empty());
    }

    #[test]
    fn test_project_scope_save_and_load_roundtrip() {
        let home = tempdir().unwrap();
        let config = Config::for_project(
            home.path().to_path_buf(),
            std::path::PathBuf::from("/Users/test/Projects/foo"),
        );

        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package("@test/project-pkg", make_v2_entry("claude", vec![]));
        cache.save(&config).unwrap();

        let expected_path = home
            .path()
            .join(".renkei/projects/Users-test-Projects-foo/install-cache.json");
        assert!(expected_path.exists());

        let loaded = InstallCache::load(&config).unwrap();
        assert_eq!(loaded.packages.len(), 1);
        assert_eq!(loaded.packages["@test/project-pkg"].version, "1.0.0");
    }

    #[test]
    fn test_none_fields_omitted_from_json() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());

        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package("@test/local-pkg", make_v2_entry("claude", vec![]));
        cache.save(&config).unwrap();

        let raw = std::fs::read_to_string(config.install_cache_path()).unwrap();
        assert!(!raw.contains("\"resolved\""));
        assert!(!raw.contains("\"tag\""));
    }

    #[test]
    fn test_save_and_load_with_hooks() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());

        let mut deployed = HashMap::new();
        deployed.insert(
            "claude".to_string(),
            BackendDeployment {
                artifacts: vec![DeployedArtifactEntry {
                    artifact_type: ArtifactKind::Hook,
                    name: "lint".to_string(),
                    deployed_path: "/home/.claude/settings.json".to_string(),
                    deployed_hooks: vec![DeployedHookEntry {
                        event: "PreToolUse".to_string(),
                        matcher: Some("bash".to_string()),
                        command: "lint.sh".to_string(),
                    }],
                    original_name: None,
                }],
                mcp_servers: vec![],
            },
        );
        let entry = PackageEntry {
            version: "1.0.0".to_string(),
            source: "local".to_string(),
            source_path: "/tmp/pkg".to_string(),
            integrity: "abc".to_string(),
            archive_path: "/tmp/a.tar.gz".to_string(),
            deployed,
            resolved: None,
            tag: None,
        };

        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package("@test/hook-pkg", entry);
        cache.save(&config).unwrap();

        let loaded = InstallCache::load(&config).unwrap();
        let hooks = &loaded.packages["@test/hook-pkg"].deployed["claude"].artifacts[0];
        assert_eq!(hooks.artifact_type, ArtifactKind::Hook);
        assert_eq!(hooks.deployed_hooks.len(), 1);
        assert_eq!(hooks.deployed_hooks[0].event, "PreToolUse");
    }

    #[test]
    fn test_original_name_roundtrip() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());

        let mut deployed = HashMap::new();
        deployed.insert(
            "claude".to_string(),
            BackendDeployment {
                artifacts: vec![DeployedArtifactEntry {
                    artifact_type: ArtifactKind::Skill,
                    name: "review-v2".to_string(),
                    deployed_path: "/p".to_string(),
                    deployed_hooks: vec![],
                    original_name: Some("review".to_string()),
                }],
                mcp_servers: vec![],
            },
        );
        let entry = PackageEntry {
            version: "1.0.0".to_string(),
            source: "local".to_string(),
            source_path: "/tmp".to_string(),
            integrity: "abc".to_string(),
            archive_path: "/tmp/a.tar.gz".to_string(),
            deployed,
            resolved: None,
            tag: None,
        };

        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package("@test/renamed", entry);
        cache.save(&config).unwrap();

        let loaded = InstallCache::load(&config).unwrap();
        let art = &loaded.packages["@test/renamed"].deployed["claude"].artifacts[0];
        assert_eq!(art.original_name.as_deref(), Some("review"));
        assert_eq!(art.name, "review-v2");
    }
}
