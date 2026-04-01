use std::path::Path;

use owo_colors::OwoColorize;

use crate::backend::Backend;
use crate::config::Config;
use crate::error::{RenkeiError, Result};
use crate::install::{self, InstallOptions, SourceKind};
use crate::manifest::RequestedScope;

pub struct WorkspaceInstallOptions {
    pub force: bool,
    pub source_kind: SourceKind,
    pub source_url: String,
    pub resolved: Option<String>,
    pub tag: Option<String>,
}

/// Install all members of a workspace.
///
/// Validates that every member directory contains a `renkei.json` before
/// installing any member (fail-fast). Then installs each member independently
/// via `install::install_local`.
pub fn install_workspace(
    workspace_dir: &Path,
    members: &[String],
    config: &Config,
    backend: &dyn Backend,
    requested_scope: RequestedScope,
    options: &WorkspaceInstallOptions,
) -> Result<()> {
    println!(
        "{} workspace with {} member(s)",
        "Detected".green().bold(),
        members.len()
    );

    // Validate all member directories exist and have a manifest before installing any
    for member in members {
        let member_dir = workspace_dir.join(member);
        if !member_dir.join("renkei.json").exists() {
            return Err(RenkeiError::ManifestNotFound(
                member_dir.join("renkei.json"),
            ));
        }
    }

    for member in members {
        let member_dir = workspace_dir.join(member);
        let member_options = build_member_options(&member_dir, options);
        install::install_local(&member_dir, config, backend, requested_scope, &member_options)?;
    }

    Ok(())
}

fn build_member_options(member_dir: &Path, ws_options: &WorkspaceInstallOptions) -> InstallOptions {
    match ws_options.source_kind {
        SourceKind::Local => InstallOptions {
            force: ws_options.force,
            ..InstallOptions::local(member_dir.to_string_lossy().to_string())
        },
        SourceKind::Git => InstallOptions {
            force: ws_options.force,
            ..InstallOptions::git(
                ws_options.source_url.clone(),
                ws_options.resolved.clone().unwrap_or_default(),
                ws_options.tag.clone(),
            )
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::claude::ClaudeBackend;
    use std::fs;
    use tempfile::tempdir;

    fn make_workspace(members: &[(&str, &str, &str)]) -> tempfile::TempDir {
        let ws = tempdir().unwrap();
        let member_names: Vec<&str> = members.iter().map(|(dir, _, _)| *dir).collect();
        let ws_json = format!(
            r#"{{ "workspace": [{}] }}"#,
            member_names
                .iter()
                .map(|n| format!("\"{n}\""))
                .collect::<Vec<_>>()
                .join(", ")
        );
        fs::write(ws.path().join("renkei.json"), ws_json).unwrap();

        for (dir_name, pkg_name, skill_name) in members {
            let member_dir = ws.path().join(dir_name);
            fs::create_dir_all(member_dir.join("skills")).unwrap();
            fs::write(
                member_dir.join("renkei.json"),
                format!(
                    r#"{{"name":"{pkg_name}","version":"1.0.0","description":"test","author":"t","license":"MIT","backends":["claude"]}}"#
                ),
            )
            .unwrap();
            fs::write(
                member_dir.join("skills").join(format!("{skill_name}.md")),
                format!("---\nname: {skill_name}\ndescription: test\n---\nContent of {skill_name}"),
            )
            .unwrap();
        }

        ws
    }

    #[test]
    fn test_install_workspace_deploys_all_members() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        let ws = make_workspace(&[
            ("member-a", "@test/member-a", "review"),
            ("member-b", "@test/member-b", "lint"),
        ]);

        let options = WorkspaceInstallOptions {
            force: false,
            source_kind: SourceKind::Local,
            source_url: ws.path().to_string_lossy().to_string(),
            resolved: None,
            tag: None,
        };

        install_workspace(
            ws.path(),
            &["member-a".to_string(), "member-b".to_string()],
            &config,
            &ClaudeBackend,
            RequestedScope::Global,
            &options,
        )
        .unwrap();

        assert!(home
            .path()
            .join(".claude/skills/renkei-review/SKILL.md")
            .exists());
        assert!(home
            .path()
            .join(".claude/skills/renkei-lint/SKILL.md")
            .exists());

        // Both should be in install-cache
        let cache = crate::install_cache::InstallCache::load(&config).unwrap();
        assert!(cache.packages.contains_key("@test/member-a"));
        assert!(cache.packages.contains_key("@test/member-b"));
    }

    #[test]
    fn test_install_workspace_missing_member_manifest_fails_before_any_install() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        let ws = tempdir().unwrap();
        fs::write(
            ws.path().join("renkei.json"),
            r#"{ "workspace": ["exists", "missing"] }"#,
        )
        .unwrap();

        // Create only 'exists' member
        let exists_dir = ws.path().join("exists");
        fs::create_dir_all(exists_dir.join("skills")).unwrap();
        fs::write(
            exists_dir.join("renkei.json"),
            r#"{"name":"@test/exists","version":"1.0.0","description":"t","author":"t","license":"MIT","backends":["claude"]}"#,
        )
        .unwrap();
        fs::write(
            exists_dir.join("skills/foo.md"),
            "---\nname: foo\ndescription: test\n---\nFoo",
        )
        .unwrap();

        // 'missing' dir exists but has no renkei.json
        fs::create_dir_all(ws.path().join("missing")).unwrap();

        let options = WorkspaceInstallOptions {
            force: false,
            source_kind: SourceKind::Local,
            source_url: ws.path().to_string_lossy().to_string(),
            resolved: None,
            tag: None,
        };

        let result = install_workspace(
            ws.path(),
            &["exists".to_string(), "missing".to_string()],
            &config,
            &ClaudeBackend,
            RequestedScope::Global,
            &options,
        );

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Manifest not found"));

        // 'exists' member should NOT have been installed (fail-fast)
        assert!(!home
            .path()
            .join(".claude/skills/renkei-foo/SKILL.md")
            .exists());
    }

    #[test]
    fn test_install_workspace_propagates_force() {
        let home = tempdir().unwrap();
        // No .claude dir — backend detection would fail without force
        let config = Config::with_home_dir(home.path().to_path_buf());

        let ws = make_workspace(&[("member-a", "@test/member-a", "review")]);

        let options = WorkspaceInstallOptions {
            force: true,
            source_kind: SourceKind::Local,
            source_url: ws.path().to_string_lossy().to_string(),
            resolved: None,
            tag: None,
        };

        let result = install_workspace(
            ws.path(),
            &["member-a".to_string()],
            &config,
            &ClaudeBackend,
            RequestedScope::Global,
            &options,
        );

        assert!(result.is_ok(), "Force flag should bypass backend detection");
    }

    #[test]
    fn test_install_workspace_each_member_has_lockfile_entry() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        let ws = make_workspace(&[
            ("member-a", "@test/member-a", "review"),
            ("member-b", "@test/member-b", "lint"),
        ]);

        let options = WorkspaceInstallOptions {
            force: false,
            source_kind: SourceKind::Local,
            source_url: ws.path().to_string_lossy().to_string(),
            resolved: None,
            tag: None,
        };

        install_workspace(
            ws.path(),
            &["member-a".to_string(), "member-b".to_string()],
            &config,
            &ClaudeBackend,
            RequestedScope::Global,
            &options,
        )
        .unwrap();

        let lockfile = crate::lockfile::Lockfile::load(&config.lockfile_path()).unwrap();
        assert!(lockfile.packages.contains_key("@test/member-a"));
        assert!(lockfile.packages.contains_key("@test/member-b"));
    }
}
