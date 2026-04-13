use std::collections::HashMap;
use std::path::{Path, PathBuf};

use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};

use crate::backend::Backend;
use crate::cache;
use crate::config::Config;
use crate::error::{RenkeiError, Result};
use crate::install;
use crate::install_cache::PackageEntry;
use crate::manifest::{self, RequestedScope};
use crate::source;

const INTEGRITY_PREFIX: &str = "sha256-";

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<String>,
}

impl Lockfile {
    /// Load from disk, returning an empty lockfile if the file does not exist.
    pub(crate) fn load(path: &Path) -> Result<Self> {
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
    pub(crate) fn load_strict(path: &Path, scope_hint: &str) -> Result<Self> {
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

    pub(crate) fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub(crate) fn upsert(&mut self, name: &str, entry: LockfileEntry) {
        self.packages.insert(name.to_string(), entry);
    }

    pub(crate) fn remove(&mut self, name: &str) {
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
            integrity: format!("{INTEGRITY_PREFIX}{}", entry.integrity),
            member: entry.member.clone(),
        }
    }
}

fn archive_path_for_entry(
    config: &Config,
    package_name: &str,
    entry: &LockfileEntry,
) -> Result<PathBuf> {
    let (scope, short_name) = manifest::parse_scoped_name(package_name)?;
    let version: semver::Version =
        entry
            .version
            .parse()
            .map_err(|e| RenkeiError::InvalidVersion {
                version: entry.version.clone(),
                reason: format!("{e}"),
            })?;
    Ok(cache::archive_path(config, &scope, &short_name, &version))
}

fn strip_integrity_prefix(integrity: &str) -> &str {
    integrity
        .strip_prefix(INTEGRITY_PREFIX)
        .unwrap_or(integrity)
}

pub fn install_from_lockfile(config: &Config, backends: &[&dyn Backend]) -> Result<()> {
    let lockfile_path = config.lockfile_path();

    if config.is_project() && !lockfile_path.exists() {
        if let Some(ref root) = config.project_root {
            if manifest::try_load_workspace(root).is_some() {
                return Err(RenkeiError::WorkspaceDetected {
                    path: root.to_string_lossy().to_string(),
                });
            }
        }
    }

    let scope_label = config.scope_label();
    let hint = if config.is_project() {
        "Use `rk install <source>` to install a package."
    } else {
        "Use `rk install -g <source>` to install a package."
    };

    let lockfile = Lockfile::load_strict(&lockfile_path, hint)?;

    if lockfile.packages.is_empty() {
        println!("{} No packages in lockfile.", "Done.".green().bold());
        return Ok(());
    }

    let requested_scope = if config.is_project() {
        RequestedScope::Project
    } else {
        RequestedScope::Global
    };

    println!(
        "{} {} package(s) from lockfile ({scope_label} scope)",
        "Restoring".green().bold(),
        lockfile.packages.len()
    );

    let mut names: Vec<&String> = lockfile.packages.keys().collect();
    names.sort();

    for name in names {
        let entry = &lockfile.packages[name];
        let archive = archive_path_for_entry(config, name, entry)?;

        match cache::compute_sha256(&archive) {
            Ok(actual_hash) => {
                let expected_hash = strip_integrity_prefix(&entry.integrity);
                if actual_hash != expected_hash {
                    return Err(RenkeiError::IntegrityMismatch {
                        package: name.clone(),
                        expected: entry.integrity.clone(),
                        actual: format!("{INTEGRITY_PREFIX}{actual_hash}"),
                    });
                }

                let tmp = tempfile::tempdir()
                    .map_err(|e| RenkeiError::CacheError(format!("Cannot create temp dir: {e}")))?;
                cache::extract_archive_to_dir(&archive, tmp.path())?;

                let source = build_source_info(entry);
                install::install_from_lock_entry(
                    tmp.path(),
                    config,
                    backends,
                    requested_scope,
                    &source,
                )?;
            }
            Err(_) => {
                let _ = install_from_source(config, backends, requested_scope, entry)?;
            }
        }
    }

    Ok(())
}

fn build_source_info(entry: &LockfileEntry) -> install::SourceInfo {
    match source::parse_source(&entry.source) {
        source::PackageSource::GitSsh(_) | source::PackageSource::GitUrl(_) => {
            install::SourceInfo {
                source_kind: install::SourceKind::Git,
                source_url: entry.source.clone(),
                resolved: entry.resolved.clone(),
                tag: entry.tag.clone(),
                member: entry.member.clone(),
            }
        }
        source::PackageSource::Local(_) => install::SourceInfo {
            source_kind: install::SourceKind::Local,
            source_url: entry.source.clone(),
            resolved: None,
            tag: None,
            member: entry.member.clone(),
        },
    }
}

fn resolve_member_root(base: &Path, member: Option<&str>) -> PathBuf {
    match member {
        Some(m) => base.join(m),
        None => base.to_path_buf(),
    }
}

fn install_from_source(
    config: &Config,
    backends: &[&dyn Backend],
    requested_scope: RequestedScope,
    entry: &LockfileEntry,
) -> Result<Option<String>> {
    let member = entry.member.as_deref();
    match source::parse_source(&entry.source) {
        source::PackageSource::GitSsh(url) | source::PackageSource::GitUrl(url) => {
            let tmp_dir = crate::git::clone_repo(&url, entry.tag.as_deref())?;
            let sha = crate::git::resolve_head(tmp_dir.path())?;
            let source = install::SourceInfo {
                source_kind: install::SourceKind::Git,
                source_url: url,
                resolved: Some(sha),
                tag: entry.tag.clone(),
                member: entry.member.clone(),
            };
            let install_root = resolve_member_root(tmp_dir.path(), member);
            install::install_from_lock_entry(
                &install_root,
                config,
                backends,
                requested_scope,
                &source,
            )
        }
        source::PackageSource::Local(path_str) => {
            let install_root = resolve_member_root(Path::new(&path_str), member);
            let source = install::SourceInfo {
                source_kind: install::SourceKind::Local,
                source_url: path_str,
                resolved: None,
                tag: None,
                member: entry.member.clone(),
            };
            install::install_from_lock_entry(
                &install_root,
                config,
                backends,
                requested_scope,
                &source,
            )
        }
    }
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
                member: None,
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
                member: None,
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
                member: None,
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
                member: None,
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
                member: None,
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
                member: None,
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
            deployed: std::collections::HashMap::new(),
            resolved: Some("abc123".to_string()),
            tag: Some("v1.0.0".to_string()),
            member: None,
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
            deployed: std::collections::HashMap::new(),
            resolved: None,
            tag: None,
            member: None,
        };
        let lockfile_entry = LockfileEntry::from_package_entry(&entry);
        assert_eq!(lockfile_entry.source, "/tmp/pkg");
        assert!(lockfile_entry.tag.is_none());
        assert!(lockfile_entry.resolved.is_none());
        assert_eq!(lockfile_entry.integrity, "sha256-aabbcc");
    }

    #[test]
    fn test_from_package_entry_with_member() {
        let entry = PackageEntry {
            version: "1.0.0".to_string(),
            source: "git".to_string(),
            source_path: "git@github.com:user/repo".to_string(),
            integrity: "abc".to_string(),
            archive_path: "/tmp/a.tar.gz".to_string(),
            deployed: std::collections::HashMap::new(),
            resolved: Some("sha".to_string()),
            tag: None,
            member: Some("mr-review".to_string()),
        };
        let lf = LockfileEntry::from_package_entry(&entry);
        assert_eq!(lf.member.as_deref(), Some("mr-review"));
    }

    #[test]
    fn test_lockfile_member_field_omitted_when_none() {
        let mut lockfile = Lockfile {
            lockfile_version: 1,
            packages: HashMap::new(),
        };
        lockfile.upsert(
            "@test/p",
            LockfileEntry {
                version: "1.0.0".to_string(),
                source: "/tmp".to_string(),
                tag: None,
                resolved: None,
                integrity: "sha256-abc".to_string(),
                member: None,
            },
        );
        let json = serde_json::to_string_pretty(&lockfile).unwrap();
        assert!(!json.contains("\"member\""));
    }

    #[test]
    fn test_lockfile_member_field_serialized_when_some() {
        let mut lockfile = Lockfile {
            lockfile_version: 1,
            packages: HashMap::new(),
        };
        lockfile.upsert(
            "@test/p",
            LockfileEntry {
                version: "1.0.0".to_string(),
                source: "/tmp".to_string(),
                tag: None,
                resolved: None,
                integrity: "sha256-abc".to_string(),
                member: Some("mr-review".to_string()),
            },
        );
        let json = serde_json::to_string_pretty(&lockfile).unwrap();
        assert!(json.contains("\"member\": \"mr-review\""));
    }

    #[test]
    fn test_lockfile_member_roundtrip_via_disk() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rk.lock");
        let mut lockfile = Lockfile {
            lockfile_version: 1,
            packages: HashMap::new(),
        };
        lockfile.upsert(
            "@test/p",
            LockfileEntry {
                version: "1.0.0".to_string(),
                source: "git@github.com:u/r".to_string(),
                tag: None,
                resolved: Some("sha".to_string()),
                integrity: "sha256-abc".to_string(),
                member: Some("auto-test".to_string()),
            },
        );
        lockfile.save(&path).unwrap();
        let loaded = Lockfile::load(&path).unwrap();
        assert_eq!(
            loaded.packages["@test/p"].member.as_deref(),
            Some("auto-test")
        );
    }

    #[test]
    fn test_lockfile_legacy_entry_without_member_deserializes_none() {
        let json = r#"{
            "lockfileVersion": 1,
            "packages": {
                "@x/y": {
                    "version": "1.0.0",
                    "source": "/tmp",
                    "integrity": "sha256-abc"
                }
            }
        }"#;
        let lf: Lockfile = serde_json::from_str(json).unwrap();
        assert!(lf.packages["@x/y"].member.is_none());
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
                member: None,
            },
        );
        let json = serde_json::to_string_pretty(&lockfile).unwrap();
        assert!(!json.contains("\"tag\""));
        assert!(!json.contains("\"resolved\""));
    }

    // --- install_from_lockfile tests ---

    use crate::backend::claude::ClaudeBackend;
    use std::fs;

    fn make_test_package(name: &str, skill_name: &str) -> tempfile::TempDir {
        let pkg = tempdir().unwrap();
        fs::write(
            pkg.path().join("renkei.json"),
            format!(
                r#"{{"name":"{name}","version":"1.0.0","description":"t","author":"t","license":"MIT","backends":["claude"]}}"#
            ),
        )
        .unwrap();
        let skill_dir = pkg.path().join("skills").join(skill_name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {skill_name}\ndescription: test\n---\nContent of {skill_name}"),
        )
        .unwrap();
        pkg
    }

    #[test]
    fn test_install_from_lockfile_with_cached_archive() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        // Step 1: install a package normally (creates archive + lockfile)
        let pkg = make_test_package("@test/restore", "review");
        let opts = install::InstallOptions::local(pkg.path().to_string_lossy().to_string());
        install::install_local(
            pkg.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts,
        )
        .unwrap();

        // Verify lockfile was created
        let lockfile_path = config.lockfile_path();
        assert!(lockfile_path.exists());

        // Step 2: delete deployed skill (simulate clean state)
        let skill_path = home.path().join(".claude/skills/renkei-review/SKILL.md");
        assert!(skill_path.exists());
        fs::remove_dir_all(home.path().join(".claude/skills")).unwrap();
        assert!(!skill_path.exists());

        // Step 3: install from lockfile
        install_from_lockfile(&config, &[&ClaudeBackend as &dyn Backend]).unwrap();

        // Verify skill is re-deployed
        assert!(skill_path.exists());
    }

    #[test]
    fn test_install_from_lockfile_integrity_mismatch() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        // Install a package to create archive + lockfile
        let pkg = make_test_package("@test/corrupt", "review");
        let opts = install::InstallOptions::local(pkg.path().to_string_lossy().to_string());
        install::install_local(
            pkg.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts,
        )
        .unwrap();

        // Corrupt the lockfile integrity
        let lockfile_path = config.lockfile_path();
        let mut lockfile = Lockfile::load(&lockfile_path).unwrap();
        let entry = lockfile.packages.get_mut("@test/corrupt").unwrap();
        entry.integrity =
            "sha256-0000000000000000000000000000000000000000000000000000000000000000".to_string();
        lockfile.save(&lockfile_path).unwrap();

        // Install from lockfile should fail with integrity error
        let result = install_from_lockfile(&config, &[&ClaudeBackend as &dyn Backend]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Integrity check failed"));
        assert!(err.contains("@test/corrupt"));
    }

    #[test]
    fn test_install_from_lockfile_no_lockfile_global() {
        let home = tempdir().unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        let result = install_from_lockfile(&config, &[&ClaudeBackend as &dyn Backend]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("No lockfile found"));
        assert!(err.contains("rk install -g <source>"));
    }

    #[test]
    fn test_install_from_lockfile_no_lockfile_project() {
        let home = tempdir().unwrap();
        let project = tempdir().unwrap();
        let config = Config::for_project(home.path().to_path_buf(), project.path().to_path_buf());

        let result = install_from_lockfile(&config, &[&ClaudeBackend as &dyn Backend]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("No lockfile found"));
        assert!(err.contains("rk install <source>"));
    }

    #[test]
    fn test_install_from_lockfile_workspace_without_lockfile_errors() {
        let home = tempdir().unwrap();
        let project = tempdir().unwrap();

        // Create a workspace renkei.json at the project root (no lockfile)
        fs::write(
            project.path().join("renkei.json"),
            r#"{ "workspace": ["member-a"] }"#,
        )
        .unwrap();

        let config = Config::for_project(home.path().to_path_buf(), project.path().to_path_buf());

        let result = install_from_lockfile(&config, &[&ClaudeBackend as &dyn Backend]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Workspace detected"));
        assert!(err.contains("rk install --link ."));
    }

    #[test]
    fn test_install_from_lockfile_local_source_fallback() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        // Create a package that stays on disk
        let pkg = make_test_package("@test/local-fb", "review");

        // Install it normally
        let opts = install::InstallOptions::local(pkg.path().to_string_lossy().to_string());
        install::install_local(
            pkg.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts,
        )
        .unwrap();

        // Delete the archive to force fallback to source
        fs::remove_dir_all(config.archives_dir()).unwrap();

        // Delete deployed files
        fs::remove_dir_all(home.path().join(".claude/skills")).unwrap();

        // Install from lockfile — should fall back to local source
        install_from_lockfile(&config, &[&ClaudeBackend as &dyn Backend]).unwrap();

        let skill_path = home.path().join(".claude/skills/renkei-review/SKILL.md");
        assert!(skill_path.exists());
    }

    #[test]
    fn test_archive_path_for_entry() {
        let config = Config::with_home_dir(PathBuf::from("/home/user"));
        let entry = LockfileEntry {
            version: "1.2.0".to_string(),
            source: "git@github.com:user/repo".to_string(),
            tag: None,
            resolved: None,
            integrity: "sha256-abc".to_string(),
            member: None,
        };
        let path = archive_path_for_entry(&config, "@test/pkg", &entry).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/home/user/.renkei/archives/@test/pkg/1.2.0.tar.gz")
        );
    }

    #[test]
    fn test_strip_integrity_prefix() {
        assert_eq!(strip_integrity_prefix("sha256-abc123"), "abc123");
        assert_eq!(strip_integrity_prefix("abc123"), "abc123");
    }
}
