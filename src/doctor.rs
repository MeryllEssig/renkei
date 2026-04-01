use std::path::Path;

use crate::artifact::ArtifactKind;
use crate::config::Config;
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
}
