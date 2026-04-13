use crate::doctor::types::{DiagnosticKind, DoctorReport, PackageDiagnostic};

fn default_statuses() -> Vec<(String, bool)> {
    vec![("claude".to_string(), true), ("agents".to_string(), true)]
}

fn no_backend_statuses() -> Vec<(String, bool)> {
    vec![("claude".to_string(), false), ("agents".to_string(), false)]
}

#[test]
fn test_format_healthy_report() {
    let report = DoctorReport {
        backend_ok: true,
        package_diagnostics: vec![PackageDiagnostic {
            package_name: "@test/pkg".to_string(),
            version: "1.0.0".to_string(),
            issues: vec![],
        }],
        local_mcp_issues: vec![],
    };
    let output = report.format("global", &default_statuses());
    assert!(output.contains("rk doctor (global)"));
    assert!(output.contains("claude"));
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
        local_mcp_issues: vec![],
    };
    let output = report.format("global", &no_backend_statuses());
    assert!(output.contains("not found"));
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
        local_mcp_issues: vec![],
    };
    let output = report.format("project", &default_statuses());
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
        local_mcp_issues: vec![],
    };
    let output = report.format("global", &default_statuses());
    assert!(output.contains("WARN"));
    assert!(output.contains("locally modified"));
}

#[test]
fn test_format_empty_packages() {
    let report = DoctorReport {
        backend_ok: true,
        package_diagnostics: vec![],
        local_mcp_issues: vec![],
    };
    let output = report.format("global", &default_statuses());
    assert!(output.contains("rk doctor (global)"));
    assert!(output.contains("All healthy: 0 package(s)."));
}

#[test]
fn test_format_per_backend_lines() {
    let statuses = vec![
        ("claude".to_string(), true),
        ("agents".to_string(), true),
        ("cursor".to_string(), false),
    ];
    let report = DoctorReport {
        backend_ok: true,
        package_diagnostics: vec![],
        local_mcp_issues: vec![],
    };
    let output = report.format("global", &statuses);
    assert!(output.contains("Backend: claude"));
    assert!(output.contains("Backend: agents"));
    assert!(output.contains("Backend: cursor"));
    assert!(output.contains("not found"));
}

#[test]
fn test_format_local_mcp_section_renders_issues() {
    let report = DoctorReport {
        backend_ok: true,
        package_diagnostics: vec![],
        local_mcp_issues: vec![
            DiagnosticKind::McpLocalMissing {
                name: "srv-a".to_string(),
            },
            DiagnosticKind::McpLocalIntegrityDrift {
                name: "srv-b".to_string(),
            },
            DiagnosticKind::McpLocalEntrypointMissing {
                name: "srv-c".to_string(),
                entrypoint: "dist/index.js".to_string(),
            },
        ],
    };
    let output = report.format("global", &default_statuses());
    assert!(output.contains("Local MCPs"));
    assert!(output.contains("srv-a"));
    assert!(output.contains("srv-b"));
    assert!(output.contains("source content changed"));
    assert!(output.contains("dist/index.js"));
}
