use crate::doctor::types::{DiagnosticKind, DoctorReport, PackageDiagnostic};

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

#[test]
fn test_build_report_runs_all_checks() {
    let entry = super::make_entry(vec![super::make_artifact(
        crate::artifact::ArtifactKind::Skill,
        "review",
        "/nonexistent/SKILL.md",
    )]);
    let settings = serde_json::json!({});
    let claude_config = serde_json::json!({});

    let packages = vec![("@test/pkg", &entry)];
    let report = DoctorReport::build(&packages, &settings, &claude_config, true);

    assert_eq!(report.package_diagnostics.len(), 1);
    // Should at least have FileMissing and ArchiveMissing
    assert!(report.package_diagnostics[0].issues.len() >= 2);
    assert!(report.package_diagnostics[0]
        .issues
        .iter()
        .any(|i| matches!(i, DiagnosticKind::FileMissing { .. })));
}
