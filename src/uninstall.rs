use owo_colors::OwoColorize;

use crate::config::Config;
use crate::error::{RenkeiError, Result};
use crate::install;
use crate::package_store::PackageStore;

pub fn run_uninstall(package: &str, config: &Config) -> Result<()> {
    let mut store = PackageStore::load(config)?;

    if !store.contains(package) {
        return Err(RenkeiError::PackageNotFound {
            package: package.to_string(),
            scope: config.scope_label().to_string(),
        });
    }

    install::cleanup_previous_installation(package, store.cache(), config);
    install::cleanup_local_mcp_refs(package, store.cache_mut(), config);

    store.remove(package);
    store.save(config)?;

    println!("{} Uninstalled {}", "Done.".green().bold(), package.bold());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::ArtifactKind;
    use crate::hook::DeployedHookEntry;
    use crate::install_cache::{
        BackendDeployment, DeployedArtifactEntry, InstallCache, PackageEntry,
    };
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn make_config_global(home: &std::path::Path) -> Config {
        Config::with_home_dir(home.to_path_buf())
    }

    fn make_config_project(home: &std::path::Path, project: &std::path::Path) -> Config {
        Config::for_project(home.to_path_buf(), project.to_path_buf())
    }

    fn make_v2_package(
        artifacts: Vec<DeployedArtifactEntry>,
        mcp_servers: Vec<String>,
    ) -> PackageEntry {
        let mut deployed = HashMap::new();
        deployed.insert(
            "claude".to_string(),
            BackendDeployment {
                artifacts,
                mcp_servers,
            },
        );
        PackageEntry {
            version: "1.0.0".to_string(),
            source: "local".to_string(),
            source_path: "/tmp/pkg".to_string(),
            integrity: "abc".to_string(),
            archive_path: "/tmp/a.tar.gz".to_string(),
            deployed,
            resolved: None,
            tag: None,
            member: None,
            mcp_local_sources: std::collections::HashMap::new(),
        }
    }

    fn make_skill_entry(skill_name: &str, deployed_path: &str) -> PackageEntry {
        make_v2_package(
            vec![DeployedArtifactEntry {
                artifact_type: ArtifactKind::Skill,
                name: skill_name.to_string(),
                deployed_path: deployed_path.to_string(),
                deployed_hooks: vec![],
                original_name: None,
            }],
            vec![],
        )
    }

    #[test]
    fn test_uninstall_removes_skill_files() {
        let home = tempdir().unwrap();
        let config = make_config_global(home.path());

        // Create the deployed skill file
        let skill_dir = home.path().join(".claude/skills/review");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_path, "# Review skill").unwrap();

        // Set up install cache
        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package(
            "@test/review",
            make_skill_entry("review", skill_path.to_str().unwrap()),
        );
        cache.save(&config).unwrap();

        // Uninstall
        run_uninstall("@test/review", &config).unwrap();

        // Verify file removed
        assert!(!skill_path.exists());
        // Verify parent dir removed too
        assert!(!skill_dir.exists());
        // Verify cache updated
        let loaded = InstallCache::load(&config).unwrap();
        assert!(!loaded.packages.contains_key("@test/review"));
    }

    #[test]
    fn test_uninstall_removes_agent_files() {
        let home = tempdir().unwrap();
        let config = make_config_global(home.path());

        let agents_dir = home.path().join(".claude/agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        let agent_path = agents_dir.join("deploy.md");
        std::fs::write(&agent_path, "# Deploy agent").unwrap();

        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package(
            "@test/deploy",
            make_v2_package(
                vec![DeployedArtifactEntry {
                    artifact_type: ArtifactKind::Agent,
                    name: "deploy".to_string(),
                    deployed_path: agent_path.to_str().unwrap().to_string(),
                    deployed_hooks: vec![],
                    original_name: None,
                }],
                vec![],
            ),
        );
        cache.save(&config).unwrap();

        run_uninstall("@test/deploy", &config).unwrap();

        assert!(!agent_path.exists());
        let loaded = InstallCache::load(&config).unwrap();
        assert!(!loaded.packages.contains_key("@test/deploy"));
    }

    #[test]
    fn test_uninstall_removes_hooks_from_settings() {
        let home = tempdir().unwrap();
        let config = make_config_global(home.path());

        // Create settings.json with a hook
        let settings_path = config
            .backend(crate::config::BackendId::Claude)
            .settings_path
            .unwrap();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        let settings = serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "bash",
                        "hooks": [
                            { "type": "command", "command": "lint.sh" }
                        ]
                    }
                ]
            }
        });
        std::fs::write(
            &settings_path,
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();

        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package(
            "@test/hooks-pkg",
            make_v2_package(
                vec![DeployedArtifactEntry {
                    artifact_type: ArtifactKind::Hook,
                    name: "lint".to_string(),
                    deployed_path: settings_path.to_str().unwrap().to_string(),
                    deployed_hooks: vec![DeployedHookEntry {
                        event: "PreToolUse".to_string(),
                        matcher: Some("bash".to_string()),
                        command: "lint.sh".to_string(),
                    }],
                    original_name: None,
                }],
                vec![],
            ),
        );
        cache.save(&config).unwrap();

        run_uninstall("@test/hooks-pkg", &config).unwrap();

        // Verify hook removed from settings
        let content = std::fs::read_to_string(&settings_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        // hooks key should be removed since it's empty
        assert!(parsed.get("hooks").is_none());
    }

    #[test]
    fn test_uninstall_removes_mcp_from_config() {
        let home = tempdir().unwrap();
        let config = make_config_global(home.path());

        // Create .claude.json with MCP servers
        let config_path = config
            .backend(crate::config::BackendId::Claude)
            .config_path
            .unwrap();
        let claude_json = serde_json::json!({
            "mcpServers": {
                "test-server": {
                    "command": "test-cmd"
                }
            }
        });
        std::fs::write(
            &config_path,
            serde_json::to_string_pretty(&claude_json).unwrap(),
        )
        .unwrap();

        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package(
            "@test/mcp-pkg",
            make_v2_package(vec![], vec!["test-server".to_string()]),
        );
        cache.save(&config).unwrap();

        run_uninstall("@test/mcp-pkg", &config).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed.get("mcpServers").is_none());
    }

    #[test]
    fn test_uninstall_package_not_found() {
        let home = tempdir().unwrap();
        let project = home.path().join("myproject");
        std::fs::create_dir_all(&project).unwrap();
        let config = make_config_project(home.path(), &project);

        let err = run_uninstall("@test/nonexistent", &config).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("@test/nonexistent"));
        assert!(msg.contains("project"));
    }

    #[test]
    fn test_uninstall_package_not_found_global() {
        let home = tempdir().unwrap();
        let config = make_config_global(home.path());

        let err = run_uninstall("@test/nonexistent", &config).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("@test/nonexistent"));
        assert!(msg.contains("global"));
    }

    #[test]
    fn test_uninstall_leaves_other_packages() {
        let home = tempdir().unwrap();
        let config = make_config_global(home.path());

        // Create skill files for both packages
        let skill_a_dir = home.path().join(".claude/skills/a");
        std::fs::create_dir_all(&skill_a_dir).unwrap();
        let skill_a = skill_a_dir.join("SKILL.md");
        std::fs::write(&skill_a, "# Skill A").unwrap();

        let skill_b_dir = home.path().join(".claude/skills/b");
        std::fs::create_dir_all(&skill_b_dir).unwrap();
        let skill_b = skill_b_dir.join("SKILL.md");
        std::fs::write(&skill_b, "# Skill B").unwrap();

        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package(
            "@test/pkg-a",
            make_skill_entry("a", skill_a.to_str().unwrap()),
        );
        cache.upsert_package(
            "@test/pkg-b",
            make_skill_entry("b", skill_b.to_str().unwrap()),
        );
        cache.save(&config).unwrap();

        // Uninstall only pkg-a
        run_uninstall("@test/pkg-a", &config).unwrap();

        // pkg-a gone
        assert!(!skill_a.exists());
        // pkg-b still there
        assert!(skill_b.exists());

        let loaded = InstallCache::load(&config).unwrap();
        assert!(!loaded.packages.contains_key("@test/pkg-a"));
        assert!(loaded.packages.contains_key("@test/pkg-b"));
    }

    #[test]
    fn test_uninstall_updates_install_cache() {
        let home = tempdir().unwrap();
        let config = make_config_global(home.path());

        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package("@test/pkg", make_v2_package(vec![], vec![]));
        cache.save(&config).unwrap();

        run_uninstall("@test/pkg", &config).unwrap();

        // Load from disk and verify
        let loaded = InstallCache::load(&config).unwrap();
        assert!(loaded.packages.is_empty());
    }

    #[test]
    fn test_uninstall_tolerates_missing_files() {
        let home = tempdir().unwrap();
        let config = make_config_global(home.path());

        // Package entry points to files that don't exist
        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package(
            "@test/ghost",
            make_skill_entry("ghost", "/nonexistent/path/SKILL.md"),
        );
        cache.save(&config).unwrap();

        // Should not error
        run_uninstall("@test/ghost", &config).unwrap();

        let loaded = InstallCache::load(&config).unwrap();
        assert!(!loaded.packages.contains_key("@test/ghost"));
    }

    #[test]
    fn test_uninstall_lockfile_removed_when_present() {
        let home = tempdir().unwrap();
        let config = make_config_global(home.path());

        // Create a lockfile with the package
        let lockfile_path = config.lockfile_path();
        std::fs::create_dir_all(lockfile_path.parent().unwrap()).unwrap();
        let lockfile = serde_json::json!({
            "lockfileVersion": 1,
            "packages": {
                "@test/pkg": {
                    "version": "1.0.0",
                    "source": "local",
                    "resolved": null,
                    "integrity": "abc"
                },
                "@test/other": {
                    "version": "2.0.0",
                    "source": "git",
                    "resolved": "sha123",
                    "integrity": "def"
                }
            }
        });
        std::fs::write(
            &lockfile_path,
            serde_json::to_string_pretty(&lockfile).unwrap(),
        )
        .unwrap();

        // Set up cache
        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package("@test/pkg", make_v2_package(vec![], vec![]));
        cache.save(&config).unwrap();

        run_uninstall("@test/pkg", &config).unwrap();

        // Verify lockfile updated
        let content = std::fs::read_to_string(&lockfile_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let packages = parsed["packages"].as_object().unwrap();
        assert!(!packages.contains_key("@test/pkg"));
        assert!(packages.contains_key("@test/other"));
    }

    #[test]
    fn test_uninstall_lockfile_skip_when_absent() {
        let home = tempdir().unwrap();
        let config = make_config_global(home.path());

        let mut cache = InstallCache::load(&config).unwrap();
        cache.upsert_package("@test/pkg", make_v2_package(vec![], vec![]));
        cache.save(&config).unwrap();

        // No lockfile exists — should succeed silently
        assert!(!config.lockfile_path().exists());
        run_uninstall("@test/pkg", &config).unwrap();
    }

    use crate::install_cache::{McpLocalEntry, McpLocalRef};

    fn setup_local_mcp_cache(
        config: &Config,
        package: &str,
        mcp_name: &str,
        refs: Vec<McpLocalRef>,
    ) {
        let mut cache = InstallCache::load(config).unwrap();
        cache.upsert_package(package, make_v2_package(vec![], vec![mcp_name.to_string()]));
        cache.mcp_local.insert(
            mcp_name.to_string(),
            McpLocalEntry {
                owner_package: package.to_string(),
                version: "1.0.0".to_string(),
                source_sha256: "sha-1".to_string(),
                referenced_by: refs,
            },
        );
        cache.save(config).unwrap();
    }

    fn write_claude_json_with_mcp(config: &Config, server_name: &str) {
        let config_path = config
            .backend(crate::config::BackendId::Claude)
            .config_path
            .unwrap();
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        let claude_json = serde_json::json!({
            "mcpServers": {
                server_name: { "command": "node", "args": ["/foo.js"] }
            }
        });
        std::fs::write(
            &config_path,
            serde_json::to_string_pretty(&claude_json).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn test_uninstall_local_mcp_decrements_ref_only_when_others_remain() {
        let home = tempdir().unwrap();
        let config = make_config_global(home.path());
        let mcp_dir = config.global_mcp_dir().join("my-srv");
        std::fs::create_dir_all(&mcp_dir).unwrap();
        std::fs::write(mcp_dir.join("dist.js"), "module.exports=1;").unwrap();

        write_claude_json_with_mcp(&config, "my-srv");
        setup_local_mcp_cache(
            &config,
            "@acme/pkg",
            "my-srv",
            vec![
                McpLocalRef {
                    package: "@acme/pkg".to_string(),
                    version: "1.0.0".to_string(),
                    scope: "global".to_string(),
                    project_root: None,
                },
                McpLocalRef {
                    package: "@acme/pkg".to_string(),
                    version: "1.0.0".to_string(),
                    scope: "project".to_string(),
                    project_root: Some("/other/project".to_string()),
                },
            ],
        );

        run_uninstall("@acme/pkg", &config).unwrap();

        // Folder still there (other ref remains)
        assert!(mcp_dir.exists());
        // Backend config still has my-srv
        let claude_json_path = config
            .backend(crate::config::BackendId::Claude)
            .config_path
            .unwrap();
        let content = std::fs::read_to_string(&claude_json_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed["mcpServers"].get("my-srv").is_some());

        // Cache: mcp_local still has entry with 1 ref
        let loaded = InstallCache::load(&config).unwrap();
        let entry = loaded.mcp_local.get("my-srv").expect("entry kept");
        assert_eq!(entry.referenced_by.len(), 1);
        assert_eq!(
            entry.referenced_by[0].project_root.as_deref(),
            Some("/other/project")
        );
    }

    #[test]
    fn test_uninstall_last_local_mcp_ref_removes_folder_and_config() {
        let home = tempdir().unwrap();
        let config = make_config_global(home.path());
        let mcp_dir = config.global_mcp_dir().join("my-srv");
        std::fs::create_dir_all(&mcp_dir).unwrap();
        std::fs::write(mcp_dir.join("dist.js"), "x").unwrap();

        write_claude_json_with_mcp(&config, "my-srv");
        setup_local_mcp_cache(
            &config,
            "@acme/pkg",
            "my-srv",
            vec![McpLocalRef {
                package: "@acme/pkg".to_string(),
                version: "1.0.0".to_string(),
                scope: "global".to_string(),
                project_root: None,
            }],
        );

        run_uninstall("@acme/pkg", &config).unwrap();

        assert!(!mcp_dir.exists(), "folder GC'd");
        let claude_json_path = config
            .backend(crate::config::BackendId::Claude)
            .config_path
            .unwrap();
        let content = std::fs::read_to_string(&claude_json_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed.get("mcpServers").is_none(), "backend entry cleaned");

        let loaded = InstallCache::load(&config).unwrap();
        assert!(!loaded.mcp_local.contains_key("my-srv"));
    }

    #[cfg(unix)]
    #[test]
    fn test_uninstall_linked_local_mcp_removes_symlink_only() {
        use std::os::unix::fs::symlink;

        let home = tempdir().unwrap();
        let config = make_config_global(home.path());

        // "Workspace" source folder — must survive uninstall.
        let workspace = home.path().join("workspace").join("mcp").join("my-srv");
        std::fs::create_dir_all(&workspace).unwrap();
        let marker = workspace.join("marker.txt");
        std::fs::write(&marker, "keep-me").unwrap();

        let link_target = config.global_mcp_dir().join("my-srv");
        std::fs::create_dir_all(link_target.parent().unwrap()).unwrap();
        symlink(&workspace, &link_target).unwrap();

        write_claude_json_with_mcp(&config, "my-srv");
        setup_local_mcp_cache(
            &config,
            "@acme/pkg",
            "my-srv",
            vec![McpLocalRef {
                package: "@acme/pkg".to_string(),
                version: "1.0.0".to_string(),
                scope: "global".to_string(),
                project_root: None,
            }],
        );

        run_uninstall("@acme/pkg", &config).unwrap();

        // Symlink removed, workspace source intact.
        assert!(!link_target.exists());
        assert!(!link_target.is_symlink());
        assert!(marker.exists(), "workspace source must not be touched");
    }

    #[test]
    fn test_uninstall_tolerates_missing_mcp_folder() {
        let home = tempdir().unwrap();
        let config = make_config_global(home.path());
        // No folder on disk, but cache has the entry.
        setup_local_mcp_cache(
            &config,
            "@acme/pkg",
            "my-srv",
            vec![McpLocalRef {
                package: "@acme/pkg".to_string(),
                version: "1.0.0".to_string(),
                scope: "global".to_string(),
                project_root: None,
            }],
        );

        run_uninstall("@acme/pkg", &config).unwrap();

        let loaded = InstallCache::load(&config).unwrap();
        assert!(!loaded.mcp_local.contains_key("my-srv"));
    }
}
