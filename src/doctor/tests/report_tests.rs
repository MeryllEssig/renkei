use crate::config::Config;
use crate::doctor::types::{DiagnosticKind, DoctorReport, PackageDiagnostic};
use crate::install_cache::InstallCache;
use std::collections::HashMap;
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
        local_mcp_issues: vec![],
    };
    assert!(report.is_healthy());
}

#[test]
fn test_report_unhealthy_backend() {
    let report = DoctorReport {
        backend_ok: false,
        package_diagnostics: vec![],
        local_mcp_issues: vec![],
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
        local_mcp_issues: vec![],
    };
    assert!(!report.is_healthy());
}

#[test]
fn test_report_unhealthy_local_mcp_error() {
    let report = DoctorReport {
        backend_ok: true,
        package_diagnostics: vec![],
        local_mcp_issues: vec![DiagnosticKind::McpLocalMissing {
            name: "srv".to_string(),
        }],
    };
    assert!(!report.is_healthy());
}

#[test]
fn test_report_healthy_despite_integrity_drift_warning() {
    let report = DoctorReport {
        backend_ok: true,
        package_diagnostics: vec![],
        local_mcp_issues: vec![DiagnosticKind::McpLocalIntegrityDrift {
            name: "srv".to_string(),
        }],
    };
    assert!(report.is_healthy(), "drift is a warning, not an error");
}

#[test]
fn test_build_report_runs_all_checks() {
    let entry = super::make_entry(vec![super::make_artifact(
        crate::artifact::ArtifactKind::Skill,
        "review",
        "/nonexistent/SKILL.md",
    )]);
    let settings = serde_json::json!({});
    let claude_config = serde_json::json!({});

    let dir = tempdir().unwrap();
    let config = Config::with_home_dir(dir.path().to_path_buf());
    let cache = InstallCache {
        version: 3,
        packages: HashMap::new(),
        mcp_local: HashMap::new(),
    };

    let packages = vec![("@test/pkg", &entry)];
    let report = DoctorReport::build(&packages, &settings, &claude_config, true, &cache, &config);

    assert_eq!(report.package_diagnostics.len(), 1);
    // Should at least have FileMissing and ArchiveMissing
    assert!(report.package_diagnostics[0].issues.len() >= 2);
    assert!(report.package_diagnostics[0]
        .issues
        .iter()
        .any(|i| matches!(i, DiagnosticKind::FileMissing { .. })));
    assert!(report.local_mcp_issues.is_empty());
}
