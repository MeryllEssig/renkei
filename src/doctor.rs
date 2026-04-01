use std::path::Path;

use owo_colors::OwoColorize;

use crate::artifact::ArtifactKind;
use crate::backend::Backend;
use crate::cache;
use crate::config::Config;
use crate::env_check;
use crate::error::Result;
use crate::install_cache::{InstallCache, PackageEntry};

#[derive(Debug, Clone, PartialEq)]
pub enum DiagnosticKind {
    FileMissing {
        artifact_name: String,
    },
    SkillModified {
        artifact_name: String,
    },
    EnvVarMissing {
        var_name: String,
        description: String,
    },
    HookMissing {
        event: String,
        command: String,
    },
    McpMissing {
        server_name: String,
    },
    ArchiveMissing {
        archive_path: String,
    },
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
        self.backend_ok && self.package_diagnostics.iter().all(|p| p.issues.is_empty())
    }
}

fn check_deployed_files(entry: &PackageEntry) -> Vec<DiagnosticKind> {
    let mut issues = Vec::new();
    for artifact in &entry.deployed_artifacts {
        match artifact.artifact_type {
            ArtifactKind::Skill | ArtifactKind::Agent => {
                if !Path::new(&artifact.deployed_path).exists() {
                    issues.push(DiagnosticKind::FileMissing {
                        artifact_name: artifact.name.clone(),
                    });
                }
            }
            ArtifactKind::Hook => {}
        }
    }
    issues
}

/// Result of reading the archive once per package — shared by skill and env checks.
enum ArchiveState {
    Available,
    Missing(String),
}

fn check_archive(entry: &PackageEntry) -> ArchiveState {
    let archive_path = Path::new(&entry.archive_path);
    if archive_path.exists() {
        ArchiveState::Available
    } else {
        ArchiveState::Missing(entry.archive_path.clone())
    }
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
            continue;
        }

        let archive_name = artifact.original_name.as_deref().unwrap_or(&artifact.name);
        let inner_path = format!("skills/{}.md", archive_name);

        let original_bytes = match cache::extract_file_from_archive(archive_path, &inner_path) {
            Ok(bytes) => bytes,
            Err(_) => break,
        };

        let original_hash = cache::compute_sha256_bytes(&original_bytes);
        let deployed_hash = match cache::compute_sha256(deployed_path) {
            Ok(h) => h,
            Err(_) => continue,
        };

        if original_hash != deployed_hash {
            issues.push(DiagnosticKind::SkillModified {
                artifact_name: artifact.name.clone(),
            });
        }
    }
    issues
}

fn check_env_vars(entry: &PackageEntry) -> Vec<DiagnosticKind> {
    let archive_path = Path::new(&entry.archive_path);

    let manifest_bytes = match cache::extract_file_from_archive(archive_path, "renkei.json") {
        Ok(bytes) => bytes,
        Err(_) => return vec![],
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

fn check_mcp(entry: &PackageEntry, claude_config: &serde_json::Value) -> Vec<DiagnosticKind> {
    let mut issues = Vec::new();
    for server_name in &entry.deployed_mcp_servers {
        let exists = claude_config
            .get("mcpServers")
            .and_then(|m| m.get(server_name))
            .is_some();
        if !exists {
            issues.push(DiagnosticKind::McpMissing {
                server_name: server_name.clone(),
            });
        }
    }
    issues
}

fn format_report(report: &DoctorReport, scope_label: &str) -> String {
    let mut out = format!("rk doctor ({scope_label})\n\n");

    // Backend line
    let backend_status = if report.backend_ok {
        format!("{}", "ok".green())
    } else {
        format!("{}", "FAIL".red().bold())
    };
    out.push_str(&format!(
        "Backend: Claude Code ............ {backend_status}\n"
    ));

    for diag in &report.package_diagnostics {
        out.push('\n');
        out.push_str(&format!("{} v{}\n", diag.package_name.bold(), diag.version));

        let (mut files, mut skills, mut envs, mut hooks, mut mcps) =
            (vec![], vec![], vec![], vec![], vec![]);
        for issue in &diag.issues {
            match issue {
                DiagnosticKind::FileMissing { .. } => files.push(issue),
                DiagnosticKind::SkillModified { .. } | DiagnosticKind::ArchiveMissing { .. } => {
                    skills.push(issue)
                }
                DiagnosticKind::EnvVarMissing { .. } => envs.push(issue),
                DiagnosticKind::HookMissing { .. } => hooks.push(issue),
                DiagnosticKind::McpMissing { .. } => mcps.push(issue),
            }
        }

        format_check_section(&mut out, "Deployed files", &files);
        format_check_section(&mut out, "Skill integrity", &skills);
        format_check_section(&mut out, "Environment variables", &envs);
        format_check_section(&mut out, "Hooks", &hooks);
        format_check_section(&mut out, "MCP servers", &mcps);
    }

    // Summary
    let total = report.package_diagnostics.len();
    let with_issues = report
        .package_diagnostics
        .iter()
        .filter(|p| !p.issues.is_empty())
        .count();
    let healthy = total - with_issues;
    out.push('\n');
    if with_issues == 0 && report.backend_ok {
        out.push_str(&format!(
            "{}",
            format!("All healthy: {total} package(s).\n").green()
        ));
    } else {
        out.push_str(&format!("{healthy} healthy, {with_issues} with issues.\n"));
    }

    out
}

fn format_check_section(out: &mut String, label: &str, issues: &[&DiagnosticKind]) {
    let dots = ".".repeat(32_usize.saturating_sub(label.len()));
    if issues.is_empty() {
        out.push_str(&format!("  {label} {dots} {}\n", "ok".green()));
    } else {
        let status_label = if issues
            .iter()
            .all(|i| matches!(i, DiagnosticKind::SkillModified { .. }))
        {
            format!("{}", "WARN".yellow().bold())
        } else {
            format!("{}", "FAIL".red().bold())
        };
        out.push_str(&format!("  {label} {dots} {status_label}\n"));

        for issue in issues {
            match issue {
                DiagnosticKind::FileMissing { artifact_name, .. } => {
                    out.push_str(&format!(
                        "    {} {} — file missing\n",
                        "x".red(),
                        artifact_name
                    ));
                }
                DiagnosticKind::SkillModified { artifact_name, .. } => {
                    out.push_str(&format!(
                        "    {} {} — locally modified\n",
                        "!".yellow(),
                        artifact_name
                    ));
                }
                DiagnosticKind::EnvVarMissing {
                    var_name,
                    description,
                } => {
                    out.push_str(&format!(
                        "    {} {} — {}\n",
                        "x".red(),
                        var_name,
                        description
                    ));
                }
                DiagnosticKind::HookMissing { event, command } => {
                    out.push_str(&format!(
                        "    {} {} ({}) — missing from settings.json\n",
                        "x".red(),
                        command,
                        event
                    ));
                }
                DiagnosticKind::McpMissing { server_name } => {
                    out.push_str(&format!(
                        "    {} {} — missing from claude.json\n",
                        "x".red(),
                        server_name
                    ));
                }
                DiagnosticKind::ArchiveMissing { archive_path } => {
                    out.push_str(&format!(
                        "    {} archive missing: {}\n",
                        "x".red(),
                        archive_path
                    ));
                }
            }
        }
    }
}

pub fn run_doctor(config: &Config, global: bool, backend: &dyn Backend) -> Result<bool> {
    let cache = InstallCache::load(config)?;
    let scope_label = if global { "global" } else { "project" };

    if cache.packages.is_empty() {
        println!("No packages installed ({scope_label}).");
        return Ok(true);
    }

    let backend_ok = backend.detect_installed(config);

    let settings = crate::json_file::read_json_or_empty(&config.claude_settings_path())?;
    let claude_config = crate::json_file::read_json_or_empty(&config.claude_config_path())?;

    let mut packages: Vec<_> = cache.packages.iter().collect();
    packages.sort_by_key(|(name, _)| name.as_str());

    let mut package_diagnostics = Vec::new();
    for (name, entry) in &packages {
        let mut issues = Vec::new();
        issues.extend(check_deployed_files(entry));
        match check_archive(entry) {
            ArchiveState::Available => {
                issues.extend(check_skill_modifications(entry));
                issues.extend(check_env_vars(entry));
            }
            ArchiveState::Missing(path) => {
                issues.push(DiagnosticKind::ArchiveMissing { archive_path: path });
            }
        }
        issues.extend(check_hooks(entry, &settings));
        issues.extend(check_mcp(entry, &claude_config));
        package_diagnostics.push(PackageDiagnostic {
            package_name: name.to_string(),
            version: entry.version.clone(),
            issues,
        });
    }

    let report = DoctorReport {
        backend_ok,
        package_diagnostics,
    };

    let output = format_report(&report, scope_label);
    print!("{}", output);

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
        assert!(
            matches!(&issues[0], DiagnosticKind::FileMissing { artifact_name, .. } if artifact_name == "review")
        );
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
        assert!(
            matches!(&issues[0], DiagnosticKind::SkillModified { artifact_name, .. } if artifact_name == "review")
        );
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
    fn test_skill_modification_missing_archive_skips() {
        let deploy = tempdir().unwrap();
        let deployed_path = deploy.path().join("SKILL.md");
        std::fs::write(&deployed_path, "# Skill").unwrap();

        let mut entry = make_entry(vec![make_artifact(
            ArtifactKind::Skill,
            "review",
            deployed_path.to_str().unwrap(),
        )]);
        entry.archive_path = "/nonexistent/archive.tar.gz".to_string();

        // ArchiveMissing is now emitted by run_doctor orchestration, not check_skill_modifications
        assert!(check_skill_modifications(&entry).is_empty());
    }

    #[test]
    fn test_check_archive_available() {
        let dir = tempdir().unwrap();
        let archive = dir.path().join("archive.tar.gz");
        std::fs::write(&archive, "fake").unwrap();

        let mut entry = make_entry(vec![]);
        entry.archive_path = archive.to_string_lossy().to_string();
        assert!(matches!(check_archive(&entry), ArchiveState::Available));
    }

    #[test]
    fn test_check_archive_missing() {
        let mut entry = make_entry(vec![]);
        entry.archive_path = "/nonexistent/archive.tar.gz".to_string();
        assert!(matches!(check_archive(&entry), ArchiveState::Missing(_)));
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
        assert!(
            matches!(&issues[0], DiagnosticKind::EnvVarMissing { var_name, description } if var_name == "RK_DOCTOR_TEST_B" && description == "API key")
        );
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

    fn make_hook_entry(event: &str, matcher: Option<&str>, command: &str) -> DeployedArtifactEntry {
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
        assert!(
            matches!(&issues[0], DiagnosticKind::HookMissing { event, command } if event == "PreToolUse" && command == "lint.sh")
        );
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

    // -- MCP presence tests --

    #[test]
    fn test_mcp_present() {
        let config = serde_json::json!({
            "mcpServers": {
                "test-server": {"command": "node", "args": ["server.js"]}
            }
        });
        let mut entry = make_entry(vec![]);
        entry.deployed_mcp_servers = vec!["test-server".to_string()];
        assert!(check_mcp(&entry, &config).is_empty());
    }

    #[test]
    fn test_mcp_missing() {
        let config = serde_json::json!({});
        let mut entry = make_entry(vec![]);
        entry.deployed_mcp_servers = vec!["test-server".to_string()];
        let issues = check_mcp(&entry, &config);
        assert_eq!(issues.len(), 1);
        assert!(
            matches!(&issues[0], DiagnosticKind::McpMissing { server_name } if server_name == "test-server")
        );
    }

    #[test]
    fn test_mcp_no_mcp_servers_key() {
        let config = serde_json::json!({"projects": {}});
        let mut entry = make_entry(vec![]);
        entry.deployed_mcp_servers = vec!["srv".to_string()];
        let issues = check_mcp(&entry, &config);
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_mcp_partial_match() {
        let config = serde_json::json!({
            "mcpServers": {
                "server-a": {"command": "a"}
            }
        });
        let mut entry = make_entry(vec![]);
        entry.deployed_mcp_servers = vec!["server-a".to_string(), "server-b".to_string()];
        let issues = check_mcp(&entry, &config);
        assert_eq!(issues.len(), 1);
        assert!(
            matches!(&issues[0], DiagnosticKind::McpMissing { server_name } if server_name == "server-b")
        );
    }

    #[test]
    fn test_mcp_no_servers_deployed() {
        let config = serde_json::json!({});
        let entry = make_entry(vec![]);
        assert!(check_mcp(&entry, &config).is_empty());
    }

    // -- Formatting tests --

    #[test]
    fn test_format_healthy_report() {
        let report = DoctorReport {
            backend_ok: true,
            package_diagnostics: vec![PackageDiagnostic {
                package_name: "@test/pkg".to_string(),
                version: "1.0.0".to_string(),
                issues: vec![],
            }],
        };
        let output = format_report(&report, "global");
        assert!(output.contains("rk doctor (global)"));
        assert!(output.contains("ok"));
        assert!(output.contains("@test/pkg"));
        assert!(output.contains("v1.0.0"));
        assert!(output.contains("All healthy: 1 package(s)."));
    }

    #[test]
    fn test_format_backend_missing() {
        let report = DoctorReport {
            backend_ok: false,
            package_diagnostics: vec![],
        };
        let output = format_report(&report, "global");
        assert!(output.contains("FAIL"));
        assert!(output.contains("0 healthy, 0 with issues"));
    }

    #[test]
    fn test_format_with_issues() {
        let report = DoctorReport {
            backend_ok: true,
            package_diagnostics: vec![PackageDiagnostic {
                package_name: "@test/pkg".to_string(),
                version: "1.0.0".to_string(),
                issues: vec![
                    DiagnosticKind::FileMissing {
                        artifact_name: "review".to_string(),
                    },
                    DiagnosticKind::McpMissing {
                        server_name: "srv".to_string(),
                    },
                ],
            }],
        };
        let output = format_report(&report, "project");
        assert!(output.contains("rk doctor (project)"));
        assert!(output.contains("review"));
        assert!(output.contains("file missing"));
        assert!(output.contains("srv"));
        assert!(output.contains("missing from claude.json"));
        assert!(output.contains("0 healthy, 1 with issues"));
    }

    #[test]
    fn test_format_skill_modified_shows_warn() {
        let report = DoctorReport {
            backend_ok: true,
            package_diagnostics: vec![PackageDiagnostic {
                package_name: "@test/pkg".to_string(),
                version: "1.0.0".to_string(),
                issues: vec![DiagnosticKind::SkillModified {
                    artifact_name: "review".to_string(),
                }],
            }],
        };
        let output = format_report(&report, "global");
        assert!(output.contains("WARN"));
        assert!(output.contains("locally modified"));
    }

    #[test]
    fn test_format_empty_packages() {
        let report = DoctorReport {
            backend_ok: true,
            package_diagnostics: vec![],
        };
        let output = format_report(&report, "global");
        assert!(output.contains("rk doctor (global)"));
        assert!(output.contains("All healthy: 0 package(s)."));
    }
}
