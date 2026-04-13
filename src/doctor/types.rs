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
    /// `~/.renkei/mcp/<name>/` folder (or symlink) no longer exists.
    McpLocalMissing {
        name: String,
    },
    /// Deployed folder's content hash no longer matches the recorded
    /// `source_sha256`. Warning-level: build drift is expected on `--link`
    /// installs and on any post-install user tampering.
    McpLocalIntegrityDrift {
        name: String,
    },
    /// Declared entrypoint file is missing inside the deployed MCP folder.
    McpLocalEntrypointMissing {
        name: String,
        entrypoint: String,
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
    pub local_mcp_issues: Vec<DiagnosticKind>,
}

impl DoctorReport {
    /// Error-level issues prevent a healthy report; integrity-drift warnings
    /// do not.
    pub fn is_healthy(&self) -> bool {
        self.backend_ok
            && self.package_diagnostics.iter().all(|p| p.issues.is_empty())
            && self
                .local_mcp_issues
                .iter()
                .all(|i| matches!(i, DiagnosticKind::McpLocalIntegrityDrift { .. }))
    }
}

/// Result of reading the archive once per package — shared by skill and env checks.
pub(crate) enum ArchiveState {
    Available,
    Missing(String),
}
