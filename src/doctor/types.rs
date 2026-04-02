/// Diagnostic issue types found during doctor checks.
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

/// All issues found for a single package.
#[derive(Debug)]
pub struct PackageDiagnostic {
    pub package_name: String,
    pub version: String,
    pub issues: Vec<DiagnosticKind>,
}

/// Overall doctor report.
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

/// Result of reading the archive once per package — shared by skill and env checks.
pub(crate) enum ArchiveState {
    Available,
    Missing(String),
}
