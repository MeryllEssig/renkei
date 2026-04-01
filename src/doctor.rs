use std::path::Path;

use crate::artifact::ArtifactKind;
use crate::cache;
use crate::config::Config;
use crate::env_check;
use crate::error::Result;
use crate::install_cache::{InstallCache, PackageEntry};

#[derive(Debug, Clone, PartialEq)]
pub enum DiagnosticKind {
    FileMissing { artifact_name: String, path: String },
    SkillModified { artifact_name: String, path: String },
    EnvVarMissing { var_name: String, description: String },
    HookMissing { event: String, command: String },
    McpMissing { server_name: String },
    ArchiveMissing { archive_path: String },
}

#[derive(Debug)]
pub struct PackageDiagnostic {
    pub package_name: String,
    pub version: String,
    pub issues: Vec<DiagnosticKind>,
}

#[derive(Debug)]
pub struct DoctorReport {
    pub backend_ok: bool,
    pub package_diagnostics: Vec<PackageDiagnostic>,
}

impl DoctorReport {
    pub fn is_healthy(&self) -> bool {
        self.backend_ok
            && self
                .package_diagnostics
                .iter()
                .all(|p| p.issues.is_empty())
    }
}

fn check_backend(config: &Config) -> bool {
    config.claude_dir().is_dir()
}

fn check_deployed_files(entry: &PackageEntry) -> Vec<DiagnosticKind> {
    let mut issues = Vec::new();
    for artifact in &entry.deployed_artifacts {
        match artifact.artifact_type {
            ArtifactKind::Skill | ArtifactKind::Agent => {
                if !Path::new(&artifact.deployed_path).exists() {
                    issues.push(DiagnosticKind::FileMissing {
                        artifact_name: artifact.name.clone(),
                        path: artifact.deployed_path.clone(),
                    });
                }
            }
            ArtifactKind::Hook => {}
        }
    }
    issues
}

fn check_skill_modifications(entry: &PackageEntry) -> Vec<DiagnosticKind> {
    let mut issues = Vec::new();
    let archive_path = Path::new(&entry.archive_path);

    for artifact in &entry.deployed_artifacts {
        if artifact.artifact_type != ArtifactKind::Skill {
            continue;
        }
        let deployed_path = Path::new(&artifact.deployed_path);
        if !deployed_path.exists() {
            continue; // already caught by check_deployed_files
        }

        let archive_name = artifact
            .original_name
            .as_deref()
            .unwrap_or(&artifact.name);
        let inner_path = format!("skills/{}.md", archive_name);

        let original_bytes = match cache::extract_file_from_archive(archive_path, &inner_path) {
            Ok(bytes) => bytes,
            Err(_) => {
                issues.push(DiagnosticKind::ArchiveMissing {
                    archive_path: entry.archive_path.clone(),
                });
                break; // no point checking more skills from this archive
            }
        };

        let original_hash = cache::compute_sha256_bytes(&original_bytes);
        let deployed_hash = match cache::compute_sha256(deployed_path) {
            Ok(h) => h,
            Err(_) => continue,
        };

        if original_hash != deployed_hash {
            issues.push(DiagnosticKind::SkillModified {
                artifact_name: artifact.name.clone(),
                path: artifact.deployed_path.clone(),
            });
        }
    }
    issues
}

fn check_env_vars(entry: &PackageEntry) -> Vec<DiagnosticKind> {
    let archive_path = Path::new(&entry.archive_path);

    let manifest_bytes = match cache::extract_file_from_archive(archive_path, "renkei.json") {
        Ok(bytes) => bytes,
        Err(_) => {
            // Archive missing already reported by check_skill_modifications
            return vec![];
        }
    };

    let manifest: serde_json::Value = match serde_json::from_slice(&manifest_bytes) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let required_env = match manifest.get("requiredEnv") {
        Some(v) => v,
        None => return vec![],
    };

    env_check::check_required_env(required_env)
        .into_iter()
        .map(|m| DiagnosticKind::EnvVarMissing {
            var_name: m.name,
            description: m.description,
        })
        .collect()
}

fn check_hooks(entry: &PackageEntry, settings: &serde_json::Value) -> Vec<DiagnosticKind> {
    let mut issues = Vec::new();
    for artifact in &entry.deployed_artifacts {
        for hook in &artifact.deployed_hooks {
            if !hook_exists_in_settings(settings, &hook.event, &hook.matcher, &hook.command) {
                issues.push(DiagnosticKind::HookMissing {
                    event: hook.event.clone(),
                    command: hook.command.clone(),
                });
            }
        }
    }
    issues
}

fn hook_exists_in_settings(
    settings: &serde_json::Value,
    event: &str,
    matcher: &Option<String>,
    command: &str,
) -> bool {
    let groups = match settings
        .get("hooks")
        .and_then(|h| h.get(event))
        .and_then(|e| e.as_array())
    {
        Some(arr) => arr,
        None => return false,
    };

    groups.iter().any(|group| {
        let group_matcher = group
            .get("matcher")
            .and_then(|m| m.as_str())
            .map(String::from);
        if group_matcher != *matcher {
            return false;
        }
        group
            .get("hooks")
            .and_then(|h| h.as_array())
            .is_some_and(|hooks| {
                hooks
                    .iter()
                    .any(|h| h.get("command").and_then(|c| c.as_str()) == Some(command))
            })
    })
}

pub fn run_doctor(config: &Config, global: bool) -> Result<bool> {
    let cache = InstallCache::load(config)?;
    let scope_label = if global { "global" } else { "project" };

    if cache.packages.is_empty() {
        println!("No packages installed ({scope_label}).");
        return Ok(true);
    }

    let backend_ok = check_backend(config);

    let report = DoctorReport {
        backend_ok,
        package_diagnostics: Vec::new(),
    };

    Ok(report.is_healthy())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hook::DeployedHookEntry;
    use crate::install_cache::DeployedArtifactEntry;
    use crate::manifest::{ManifestScope, ValidatedManifest};
    use semver::Version;
    use tempfile::tempdir;

    #[test]
    fn test_check_backend_exists() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());
        assert!(check_backend(&config));
    }

    #[test]
    fn test_check_backend_missing() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());
        assert!(!check_backend(&config));
    }

    #[test]
    fn test_report_healthy() {
        let report = DoctorReport {
            backend_ok: true,
            package_diagnostics: vec![PackageDiagnostic {
                package_name: "@test/pkg".to_string(),
                version: "1.0.0".to_string(),
                issues: vec![],
            }],
        };
        assert!(report.is_healthy());
    }

    #[test]
    fn test_report_unhealthy_backend() {
        let report = DoctorReport {
            backend_ok: false,
            package_diagnostics: vec![],
        };
        assert!(!report.is_healthy());
    }

    #[test]
    fn test_report_unhealthy_package_issues() {
        let report = DoctorReport {
            backend_ok: true,
            package_diagnostics: vec![PackageDiagnostic {
                package_name: "@test/pkg".to_string(),
                version: "1.0.0".to_string(),
                issues: vec![DiagnosticKind::McpMissing {
                    server_name: "srv".to_string(),
                }],
            }],
        };
        assert!(!report.is_healthy());
    }

    fn make_entry(artifacts: Vec<DeployedArtifactEntry>) -> PackageEntry {
        PackageEntry {
            version: "1.0.0".to_string(),
            source: "local".to_string(),
            source_path: "/tmp/pkg".to_string(),
            integrity: "abc".to_string(),
            archive_path: "/tmp/a.tar.gz".to_string(),
            deployed_artifacts: artifacts,
            deployed_mcp_servers: vec![],
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
    fn test_deployed_files_all_exist() {
        let dir = tempdir().unwrap();
        let skill_path = dir.path().join("skill.md");
        let agent_path = dir.path().join("agent.md");
        std::fs::write(&skill_path, "# Skill").unwrap();
        std::fs::write(&agent_path, "# Agent").unwrap();

        let entry = make_entry(vec![
            make_artifact(ArtifactKind::Skill, "review", skill_path.to_str().unwrap()),
            make_artifact(ArtifactKind::Agent, "deploy", agent_path.to_str().unwrap()),
        ]);
        assert!(check_deployed_files(&entry).is_empty());
    }

    #[test]
    fn test_deployed_files_missing() {
        let entry = make_entry(vec![make_artifact(
            ArtifactKind::Skill,
            "review",
            "/nonexistent/path/SKILL.md",
        )]);
        let issues = check_deployed_files(&entry);
        assert_eq!(issues.len(), 1);
        assert!(matches!(&issues[0], DiagnosticKind::FileMissing { artifact_name, .. } if artifact_name == "review"));
    }

    #[test]
    fn test_deployed_files_hook_skipped() {
        let entry = make_entry(vec![DeployedArtifactEntry {
            artifact_type: ArtifactKind::Hook,
            name: "lint".to_string(),
            deployed_path: "/nonexistent/settings.json".to_string(),
            deployed_hooks: vec![DeployedHookEntry {
                event: "PreToolUse".to_string(),
                matcher: Some("bash".to_string()),
                command: "lint.sh".to_string(),
            }],
            original_name: None,
        }]);
        assert!(check_deployed_files(&entry).is_empty());
    }

    #[test]
    fn test_deployed_files_multiple_missing() {
        let entry = make_entry(vec![
            make_artifact(ArtifactKind::Skill, "a", "/missing/a"),
            make_artifact(ArtifactKind::Agent, "b", "/missing/b"),
        ]);
        let issues = check_deployed_files(&entry);
        assert_eq!(issues.len(), 2);
    }

    // -- Skill modification tests --

    fn make_test_manifest() -> ValidatedManifest {
        ValidatedManifest {
            scope: "test".to_string(),
            short_name: "sample".to_string(),
            full_name: "@test/sample".to_string(),
            version: Version::new(0, 1, 0),
            install_scope: ManifestScope::Any,
            description: "test".to_string(),
            author: "tester".to_string(),
            license: "MIT".to_string(),
            backends: vec!["claude".to_string()],
        }
    }

    fn setup_package_with_skill(dir: &std::path::Path, skill_name: &str, content: &str) {
        std::fs::write(
            dir.join("renkei.json"),
            r#"{"name":"@test/sample","version":"0.1.0","description":"test","author":"tester","license":"MIT","backends":["claude"]}"#,
        ).unwrap();
        let skills = dir.join("skills");
        std::fs::create_dir_all(&skills).unwrap();
        std::fs::write(skills.join(format!("{skill_name}.md")), content).unwrap();
    }

    #[test]
    fn test_skill_unmodified() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();
        let deploy = tempdir().unwrap();

        setup_package_with_skill(pkg.path(), "review", "# Review skill");
        let config = Config::with_home_dir(home.path().to_path_buf());
        let manifest = make_test_manifest();
        let (archive_path, _) = cache::create_archive(pkg.path(), &manifest, &config).unwrap();

        // Deploy the same content
        let deployed_path = deploy.path().join("SKILL.md");
        std::fs::write(&deployed_path, "# Review skill").unwrap();

        let mut entry = make_entry(vec![make_artifact(
            ArtifactKind::Skill,
            "review",
            deployed_path.to_str().unwrap(),
        )]);
        entry.archive_path = archive_path.to_string_lossy().to_string();

        assert!(check_skill_modifications(&entry).is_empty());
    }

    #[test]
    fn test_skill_modified() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();
        let deploy = tempdir().unwrap();

        setup_package_with_skill(pkg.path(), "review", "# Review skill");
        let config = Config::with_home_dir(home.path().to_path_buf());
        let manifest = make_test_manifest();
        let (archive_path, _) = cache::create_archive(pkg.path(), &manifest, &config).unwrap();

        // Deploy different content
        let deployed_path = deploy.path().join("SKILL.md");
        std::fs::write(&deployed_path, "# Modified review skill").unwrap();

        let mut entry = make_entry(vec![make_artifact(
            ArtifactKind::Skill,
            "review",
            deployed_path.to_str().unwrap(),
        )]);
        entry.archive_path = archive_path.to_string_lossy().to_string();

        let issues = check_skill_modifications(&entry);
        assert_eq!(issues.len(), 1);
        assert!(matches!(&issues[0], DiagnosticKind::SkillModified { artifact_name, .. } if artifact_name == "review"));
    }

    #[test]
    fn test_skill_modification_missing_file_skipped() {
        let entry = make_entry(vec![make_artifact(
            ArtifactKind::Skill,
            "review",
            "/nonexistent/SKILL.md",
        )]);
        // Missing file is skipped (caught by check_deployed_files)
        assert!(check_skill_modifications(&entry).is_empty());
    }

    #[test]
    fn test_skill_modification_missing_archive() {
        let deploy = tempdir().unwrap();
        let deployed_path = deploy.path().join("SKILL.md");
        std::fs::write(&deployed_path, "# Skill").unwrap();

        let mut entry = make_entry(vec![make_artifact(
            ArtifactKind::Skill,
            "review",
            deployed_path.to_str().unwrap(),
        )]);
        entry.archive_path = "/nonexistent/archive.tar.gz".to_string();

        let issues = check_skill_modifications(&entry);
        assert_eq!(issues.len(), 1);
        assert!(matches!(&issues[0], DiagnosticKind::ArchiveMissing { .. }));
    }

    #[test]
    fn test_skill_modification_uses_original_name() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();
        let deploy = tempdir().unwrap();

        setup_package_with_skill(pkg.path(), "review", "# Review skill");
        let config = Config::with_home_dir(home.path().to_path_buf());
        let manifest = make_test_manifest();
        let (archive_path, _) = cache::create_archive(pkg.path(), &manifest, &config).unwrap();

        // Deploy same content but under a renamed name
        let deployed_path = deploy.path().join("SKILL.md");
        std::fs::write(&deployed_path, "# Review skill").unwrap();

        let mut entry = make_entry(vec![DeployedArtifactEntry {
            artifact_type: ArtifactKind::Skill,
            name: "review-v2".to_string(),
            deployed_path: deployed_path.to_string_lossy().to_string(),
            deployed_hooks: vec![],
            original_name: Some("review".to_string()),
        }]);
        entry.archive_path = archive_path.to_string_lossy().to_string();

        assert!(check_skill_modifications(&entry).is_empty());
    }

    #[test]
    fn test_skill_modification_agent_skipped() {
        let mut entry = make_entry(vec![make_artifact(
            ArtifactKind::Agent,
            "deploy",
            "/nonexistent/agent.md",
        )]);
        entry.archive_path = "/nonexistent/archive.tar.gz".to_string();
        // Agent should be skipped entirely
        assert!(check_skill_modifications(&entry).is_empty());
    }

    // -- Environment variable tests --

    fn setup_package_with_env(dir: &std::path::Path, required_env: &str) {
        std::fs::write(
            dir.join("renkei.json"),
            format!(
                r#"{{"name":"@test/sample","version":"0.1.0","description":"test","author":"tester","license":"MIT","backends":["claude"],"requiredEnv":{required_env}}}"#
            ),
        ).unwrap();
        let skills = dir.join("skills");
        std::fs::create_dir_all(&skills).unwrap();
        std::fs::write(skills.join("review.md"), "# Review").unwrap();
    }

    #[test]
    fn test_env_vars_all_present() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        unsafe { std::env::set_var("RK_DOCTOR_TEST_A", "val") };
        setup_package_with_env(pkg.path(), r#"{"RK_DOCTOR_TEST_A":"desc"}"#);
        let config = Config::with_home_dir(home.path().to_path_buf());
        let manifest = make_test_manifest();
        let (archive_path, _) = cache::create_archive(pkg.path(), &manifest, &config).unwrap();

        let mut entry = make_entry(vec![]);
        entry.archive_path = archive_path.to_string_lossy().to_string();

        let issues = check_env_vars(&entry);
        assert!(issues.is_empty());
        unsafe { std::env::remove_var("RK_DOCTOR_TEST_A") };
    }

    #[test]
    fn test_env_vars_missing() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        std::env::remove_var("RK_DOCTOR_TEST_B");
        setup_package_with_env(pkg.path(), r#"{"RK_DOCTOR_TEST_B":"API key"}"#);
        let config = Config::with_home_dir(home.path().to_path_buf());
        let manifest = make_test_manifest();
        let (archive_path, _) = cache::create_archive(pkg.path(), &manifest, &config).unwrap();

        let mut entry = make_entry(vec![]);
        entry.archive_path = archive_path.to_string_lossy().to_string();

        let issues = check_env_vars(&entry);
        assert_eq!(issues.len(), 1);
        assert!(matches!(&issues[0], DiagnosticKind::EnvVarMissing { var_name, description } if var_name == "RK_DOCTOR_TEST_B" && description == "API key"));
    }

    #[test]
    fn test_env_vars_no_required_env() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        setup_package_with_skill(pkg.path(), "review", "# Review");
        let config = Config::with_home_dir(home.path().to_path_buf());
        let manifest = make_test_manifest();
        let (archive_path, _) = cache::create_archive(pkg.path(), &manifest, &config).unwrap();

        let mut entry = make_entry(vec![]);
        entry.archive_path = archive_path.to_string_lossy().to_string();

        assert!(check_env_vars(&entry).is_empty());
    }

    #[test]
    fn test_env_vars_archive_missing() {
        let mut entry = make_entry(vec![]);
        entry.archive_path = "/nonexistent/archive.tar.gz".to_string();
        // Missing archive → empty (already reported elsewhere)
        assert!(check_env_vars(&entry).is_empty());
    }

    // -- Hook presence tests --

    fn make_hook_entry(
        event: &str,
        matcher: Option<&str>,
        command: &str,
    ) -> DeployedArtifactEntry {
        DeployedArtifactEntry {
            artifact_type: ArtifactKind::Hook,
            name: "hook".to_string(),
            deployed_path: "/settings.json".to_string(),
            deployed_hooks: vec![DeployedHookEntry {
                event: event.to_string(),
                matcher: matcher.map(String::from),
                command: command.to_string(),
            }],
            original_name: None,
        }
    }

    #[test]
    fn test_hooks_present() {
        let settings = serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "bash",
                    "hooks": [{"type": "command", "command": "lint.sh"}]
                }]
            }
        });
        let entry = make_entry(vec![make_hook_entry("PreToolUse", Some("bash"), "lint.sh")]);
        assert!(check_hooks(&entry, &settings).is_empty());
    }

    #[test]
    fn test_hooks_missing() {
        let settings = serde_json::json!({});
        let entry = make_entry(vec![make_hook_entry("PreToolUse", Some("bash"), "lint.sh")]);
        let issues = check_hooks(&entry, &settings);
        assert_eq!(issues.len(), 1);
        assert!(matches!(&issues[0], DiagnosticKind::HookMissing { event, command } if event == "PreToolUse" && command == "lint.sh"));
    }

    #[test]
    fn test_hooks_without_matcher() {
        let settings = serde_json::json!({
            "hooks": {
                "Stop": [{
                    "hooks": [{"type": "command", "command": "cleanup.sh"}]
                }]
            }
        });
        let entry = make_entry(vec![make_hook_entry("Stop", None, "cleanup.sh")]);
        assert!(check_hooks(&entry, &settings).is_empty());
    }

    #[test]
    fn test_hooks_wrong_command() {
        let settings = serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "bash",
                    "hooks": [{"type": "command", "command": "other.sh"}]
                }]
            }
        });
        let entry = make_entry(vec![make_hook_entry("PreToolUse", Some("bash"), "lint.sh")]);
        let issues = check_hooks(&entry, &settings);
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_hooks_wrong_matcher() {
        let settings = serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Write",
                    "hooks": [{"type": "command", "command": "lint.sh"}]
                }]
            }
        });
        let entry = make_entry(vec![make_hook_entry("PreToolUse", Some("bash"), "lint.sh")]);
        let issues = check_hooks(&entry, &settings);
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_hooks_no_hooks_key() {
        let settings = serde_json::json!({"language": "French"});
        let entry = make_entry(vec![make_hook_entry("PreToolUse", Some("bash"), "lint.sh")]);
        let issues = check_hooks(&entry, &settings);
        assert_eq!(issues.len(), 1);
    }
}
