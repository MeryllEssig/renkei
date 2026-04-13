use std::path::Path;

use owo_colors::OwoColorize;

use crate::backend::Backend;
use crate::config::Config;
use crate::error::{RenkeiError, Result};
use crate::install::{self, InstallOptions, SourceKind};
use crate::manifest::RequestedScope;

/// Install all (or a selected subset of) members of a workspace.
///
/// `selected = None`: install every member declared in `members`.
/// `selected = Some(names)`: install only the named members, in the order
/// requested. Every requested name must exist in `members` (fail-fast → no
/// install runs if validation fails). Validates that every member-to-install
/// directory contains a `renkei.json` before installing any of them.
pub fn install_workspace(
    workspace_dir: &Path,
    members: &[String],
    config: &Config,
    backends: &[&dyn Backend],
    requested_scope: RequestedScope,
    options: &InstallOptions,
    selected: Option<&[String]>,
) -> Result<()> {
    let to_install: Vec<String> = match selected {
        None => members.to_vec(),
        Some(requested) => {
            let mut deduped: Vec<String> = Vec::with_capacity(requested.len());
            for name in requested {
                if !members.iter().any(|m| m == name) {
                    return Err(RenkeiError::MemberNotInWorkspace {
                        requested: name.clone(),
                        available: members.to_vec(),
                    });
                }
                if !deduped.iter().any(|n| n == name) {
                    deduped.push(name.clone());
                }
            }
            deduped
        }
    };

    println!(
        "{} workspace with {} member(s)",
        "Detected".green().bold(),
        to_install.len()
    );

    for member in &to_install {
        let member_dir = workspace_dir.join(member);
        if !member_dir.join("renkei.json").exists() {
            return Err(RenkeiError::ManifestNotFound(
                member_dir.join("renkei.json"),
            ));
        }
    }

    for member in &to_install {
        let member_dir = workspace_dir.join(member);
        let member_options = build_member_options(&member_dir, member, options);
        install::install_local(
            &member_dir,
            config,
            backends,
            requested_scope,
            &member_options,
        )?;
    }

    Ok(())
}

fn build_member_options(
    member_dir: &Path,
    member_name: &str,
    base: &InstallOptions,
) -> InstallOptions {
    let mut opts = base.clone();
    if opts.source_kind == SourceKind::Local {
        opts.source_url = member_dir.to_string_lossy().to_string();
    }
    opts.member = Some(member_name.to_string());
    opts
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
            let skill_dir = member_dir.join("skills").join(skill_name);
            fs::create_dir_all(&skill_dir).unwrap();
            fs::write(
                member_dir.join("renkei.json"),
                format!(
                    r#"{{"name":"{pkg_name}","version":"1.0.0","description":"test","author":"t","license":"MIT","backends":["claude"]}}"#
                ),
            )
            .unwrap();
            fs::write(
                skill_dir.join("SKILL.md"),
                format!("---\nname: {skill_name}\ndescription: test\n---\nContent of {skill_name}"),
            )
            .unwrap();
        }

        ws
    }

    fn local_options(source_url: &str) -> InstallOptions {
        InstallOptions::local(source_url.to_string())
    }

    fn force_local_options(source_url: &str) -> InstallOptions {
        InstallOptions {
            force: true,
            ..InstallOptions::local(source_url.to_string())
        }
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

        let options = local_options(&ws.path().to_string_lossy());

        install_workspace(
            ws.path(),
            &["member-a".to_string(), "member-b".to_string()],
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &options,
            None,
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

        let exists_dir = ws.path().join("exists");
        let foo_dir = exists_dir.join("skills/foo");
        fs::create_dir_all(&foo_dir).unwrap();
        fs::write(
            exists_dir.join("renkei.json"),
            r#"{"name":"@test/exists","version":"1.0.0","description":"t","author":"t","license":"MIT","backends":["claude"]}"#,
        )
        .unwrap();
        fs::write(
            foo_dir.join("SKILL.md"),
            "---\nname: foo\ndescription: test\n---\nFoo",
        )
        .unwrap();

        fs::create_dir_all(ws.path().join("missing")).unwrap();

        let options = local_options(&ws.path().to_string_lossy());

        let result = install_workspace(
            ws.path(),
            &["exists".to_string(), "missing".to_string()],
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &options,
            None,
        );

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Manifest not found"));

        assert!(!home
            .path()
            .join(".claude/skills/renkei-foo/SKILL.md")
            .exists());
    }

    #[test]
    fn test_install_workspace_propagates_force() {
        let home = tempdir().unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        let ws = make_workspace(&[("member-a", "@test/member-a", "review")]);

        let options = force_local_options(&ws.path().to_string_lossy());

        let result = install_workspace(
            ws.path(),
            &["member-a".to_string()],
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &options,
            None,
        );

        assert!(result.is_ok(), "Force flag should bypass backend detection");
    }

    #[test]
    fn test_install_workspace_selected_subset_only_installs_named() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        let ws = make_workspace(&[
            ("member-a", "@test/member-a", "review"),
            ("member-b", "@test/member-b", "lint"),
        ]);

        let options = local_options(&ws.path().to_string_lossy());

        install_workspace(
            ws.path(),
            &["member-a".to_string(), "member-b".to_string()],
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &options,
            Some(&["member-a".to_string()]),
        )
        .unwrap();

        assert!(home
            .path()
            .join(".claude/skills/renkei-review/SKILL.md")
            .exists());
        assert!(!home
            .path()
            .join(".claude/skills/renkei-lint/SKILL.md")
            .exists());

        let cache = crate::install_cache::InstallCache::load(&config).unwrap();
        assert!(cache.packages.contains_key("@test/member-a"));
        assert!(!cache.packages.contains_key("@test/member-b"));
    }

    #[test]
    fn test_install_workspace_unknown_member_fails_before_any_install() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        let ws = make_workspace(&[
            ("member-a", "@test/member-a", "review"),
            ("member-b", "@test/member-b", "lint"),
        ]);

        let options = local_options(&ws.path().to_string_lossy());

        let result = install_workspace(
            ws.path(),
            &["member-a".to_string(), "member-b".to_string()],
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &options,
            Some(&["member-a".to_string(), "bogus".to_string()]),
        );

        assert!(matches!(
            result,
            Err(RenkeiError::MemberNotInWorkspace { .. })
        ));
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("bogus"));
        assert!(msg.contains("member-a"));
        assert!(msg.contains("member-b"));
        assert!(!home
            .path()
            .join(".claude/skills/renkei-review/SKILL.md")
            .exists());
    }

    #[test]
    fn test_install_workspace_selected_dedups_repeated_names() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        let ws = make_workspace(&[("member-a", "@test/member-a", "review")]);
        let options = local_options(&ws.path().to_string_lossy());

        install_workspace(
            ws.path(),
            &["member-a".to_string()],
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &options,
            Some(&["member-a".to_string(), "member-a".to_string()]),
        )
        .unwrap();

        let cache = crate::install_cache::InstallCache::load(&config).unwrap();
        assert!(cache.packages.contains_key("@test/member-a"));
    }

    #[test]
    fn test_install_workspace_writes_member_into_lockfile() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        let ws = make_workspace(&[
            ("member-a", "@test/member-a", "review"),
            ("member-b", "@test/member-b", "lint"),
        ]);

        let options = local_options(&ws.path().to_string_lossy());

        install_workspace(
            ws.path(),
            &["member-a".to_string(), "member-b".to_string()],
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &options,
            None,
        )
        .unwrap();

        let lockfile = crate::lockfile::Lockfile::load(&config.lockfile_path()).unwrap();
        assert_eq!(
            lockfile.packages["@test/member-a"].member.as_deref(),
            Some("member-a")
        );
        assert_eq!(
            lockfile.packages["@test/member-b"].member.as_deref(),
            Some("member-b")
        );
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

        let options = local_options(&ws.path().to_string_lossy());

        install_workspace(
            ws.path(),
            &["member-a".to_string(), "member-b".to_string()],
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &options,
            None,
        )
        .unwrap();

        let lockfile = crate::lockfile::Lockfile::load(&config.lockfile_path()).unwrap();
        assert!(lockfile.packages.contains_key("@test/member-a"));
        assert!(lockfile.packages.contains_key("@test/member-b"));
    }
}
