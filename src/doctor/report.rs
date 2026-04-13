use owo_colors::OwoColorize;

use crate::config::Config;
use crate::install_cache::{InstallCache, PackageEntry};

use super::checks;
use super::types::{ArchiveState, DiagnosticKind, DoctorReport, PackageDiagnostic};

impl DoctorReport {
    /// Build a report by running all checks on every package.
    pub fn build(
        packages: &[(&str, &PackageEntry)],
        settings: &serde_json::Value,
        claude_config: &serde_json::Value,
        backend_ok: bool,
        install_cache: &InstallCache,
        config: &Config,
    ) -> Self {
        let mut package_diagnostics = Vec::new();

        for (name, entry) in packages {
            let mut issues = Vec::new();
            issues.extend(checks::check_deployed_files(entry));
            match checks::check_archive(entry) {
                ArchiveState::Available => {
                    issues.extend(checks::check_skill_modifications(entry));
                    issues.extend(checks::check_env_vars(entry));
                }
                ArchiveState::Missing(path) => {
                    issues.push(DiagnosticKind::ArchiveMissing { archive_path: path });
                }
            }
            issues.extend(checks::check_hooks(entry, settings));
            issues.extend(checks::check_mcp(entry, claude_config));
            package_diagnostics.push(PackageDiagnostic {
                package_name: name.to_string(),
                version: entry.version.clone(),
                issues,
            });
        }

        let local_mcp_issues = checks::check_mcp_local(install_cache, config);

        DoctorReport {
            backend_ok,
            package_diagnostics,
            local_mcp_issues,
        }
    }

    /// Format the report as a human-readable string.
    pub fn format(&self, scope_label: &str, backend_statuses: &[(String, bool)]) -> String {
        let mut out = format!("rk doctor ({scope_label})\n\n");

        // Backend lines
        for (name, detected) in backend_statuses {
            let status = if *detected {
                format!("{}", "ok".green())
            } else {
                format!("{}", "not found".dimmed())
            };
            let dots = ".".repeat(28_usize.saturating_sub(name.len()));
            out.push_str(&format!("Backend: {name} {dots} {status}\n"));
        }

        for diag in &self.package_diagnostics {
            out.push('\n');
            out.push_str(&format!("{} v{}\n", diag.package_name.bold(), diag.version));

            let (mut files, mut skills, mut envs, mut hooks, mut mcps) =
                (vec![], vec![], vec![], vec![], vec![]);
            for issue in &diag.issues {
                match issue {
                    DiagnosticKind::FileMissing { .. } => files.push(issue),
                    DiagnosticKind::SkillModified { .. }
                    | DiagnosticKind::ArchiveMissing { .. } => skills.push(issue),
                    DiagnosticKind::EnvVarMissing { .. } => envs.push(issue),
                    DiagnosticKind::HookMissing { .. } => hooks.push(issue),
                    DiagnosticKind::McpMissing { .. } => mcps.push(issue),
                    DiagnosticKind::McpLocalMissing { .. }
                    | DiagnosticKind::McpLocalIntegrityDrift { .. }
                    | DiagnosticKind::McpLocalEntrypointMissing { .. } => {
                        // Local MCP issues are reported in their own global section.
                    }
                }
            }

            format_check_section(&mut out, "Deployed files", &files);
            format_check_section(&mut out, "Skill integrity", &skills);
            format_check_section(&mut out, "Environment variables", &envs);
            format_check_section(&mut out, "Hooks", &hooks);
            format_check_section(&mut out, "MCP servers", &mcps);
        }

        if !self.local_mcp_issues.is_empty() {
            out.push('\n');
            out.push_str(&format!("{}\n", "Local MCPs".bold()));
            let refs: Vec<&DiagnosticKind> = self.local_mcp_issues.iter().collect();
            format_check_section(&mut out, "Integrity", &refs);
        }

        // Summary
        let total = self.package_diagnostics.len();
        let with_issues = self
            .package_diagnostics
            .iter()
            .filter(|p| !p.issues.is_empty())
            .count();
        let healthy = total - with_issues;
        out.push('\n');
        if self.is_healthy() {
            out.push_str(&format!(
                "{}",
                format!("All healthy: {total} package(s).\n").green()
            ));
        } else {
            out.push_str(&format!("{healthy} healthy, {with_issues} with issues.\n"));
        }

        out
    }
}

fn format_check_section(out: &mut String, label: &str, issues: &[&DiagnosticKind]) {
    let dots = ".".repeat(32_usize.saturating_sub(label.len()));
    if issues.is_empty() {
        out.push_str(&format!("  {label} {dots} {}\n", "ok".green()));
    } else {
        let status_label = if issues.iter().all(|i| {
            matches!(
                i,
                DiagnosticKind::SkillModified { .. }
                    | DiagnosticKind::McpLocalIntegrityDrift { .. }
            )
        }) {
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
                DiagnosticKind::McpLocalMissing { name } => {
                    out.push_str(&format!(
                        "    {} {} — ~/.renkei/mcp/{}/ is missing\n",
                        "x".red(),
                        name,
                        name
                    ));
                }
                DiagnosticKind::McpLocalIntegrityDrift { name } => {
                    out.push_str(&format!(
                        "    {} {} — source content changed since install\n",
                        "!".yellow(),
                        name
                    ));
                }
                DiagnosticKind::McpLocalEntrypointMissing { name, entrypoint } => {
                    out.push_str(&format!(
                        "    {} {} — entrypoint `{}` missing\n",
                        "x".red(),
                        name,
                        entrypoint
                    ));
                }
            }
        }
    }
}
