use std::path::Path;

use crate::config::Config;
use crate::error::Result;
use crate::install_cache::InstallCache;

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
    use std::path::PathBuf;
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
}
