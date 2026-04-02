use owo_colors::OwoColorize;

use crate::artifact::ArtifactKind;
use crate::config::Config;
use crate::error::Result;
use crate::install_cache::{InstallCache, PackageEntry};

pub fn run_list(config: &Config, global: bool) -> Result<()> {
    let cache = InstallCache::load(config)?;
    let output = format_package_list(&cache, global);
    print!("{}", output);
    Ok(())
}

pub fn format_package_list(cache: &InstallCache, global: bool) -> String {
    let scope_label = if global { "global" } else { "project" };

    if cache.packages.is_empty() {
        return format!("No packages installed ({scope_label}).\n");
    }

    let mut output = format!("Installed packages ({scope_label}):\n\n");

    let mut packages: Vec<_> = cache.packages.iter().collect();
    packages.sort_by_key(|(name, _)| name.as_str());

    for (i, (name, entry)) in packages.iter().enumerate() {
        output.push_str(&format_package_line(name, entry));
        output.push('\n');
        output.push_str(&format_artifact_lines(entry));
        output.push('\n');
        if i < packages.len() - 1 {
            output.push('\n');
        }
    }

    output
}

fn format_package_line(name: &str, entry: &PackageEntry) -> String {
    let source_badge = if entry.source == "git" {
        format!("{}", "[git]".cyan())
    } else {
        format!("{}", "[local]".dimmed())
    };

    let git_detail = if entry.source == "git" {
        let sha_short = entry
            .resolved
            .as_deref()
            .map(|s| s.get(..7).unwrap_or(s))
            .unwrap_or("unknown");
        match &entry.tag {
            Some(tag) => format!(" {}", format!("({tag} @ {sha_short})").dimmed()),
            None => format!(" {}", format!("(@ {sha_short})").dimmed()),
        }
    } else {
        String::new()
    };

    format!(
        "  {} v{} {source_badge}{git_detail}",
        name.bold(),
        entry.version
    )
}

fn format_artifact_lines(entry: &PackageEntry) -> String {
    let mut lines = Vec::new();

    let mut backend_names: Vec<&String> = entry.deployed.keys().collect();
    backend_names.sort();

    for backend_name in backend_names {
        if let Some(deployment) = entry.deployed.get(backend_name) {
            let summary = format_backend_summary(deployment);
            if !summary.is_empty() {
                lines.push(format!(
                    "    {} {}: {summary}",
                    "→".dimmed(),
                    backend_name.dimmed()
                ));
            }
        }
    }

    if lines.is_empty() {
        format!("    {} {}", "→".dimmed(), "(no artifacts)".dimmed())
    } else {
        lines.join("\n")
    }
}

fn format_backend_summary(deployment: &crate::install_cache::BackendDeployment) -> String {
    let (mut skills, mut agents, mut hooks) = (0, 0, 0);
    for a in &deployment.artifacts {
        match a.artifact_type {
            ArtifactKind::Skill => skills += 1,
            ArtifactKind::Agent => agents += 1,
            ArtifactKind::Hook => hooks += 1,
        }
    }
    let mcp = deployment.mcp_servers.len();

    let mut parts = Vec::new();
    if skills > 0 {
        parts.push(pluralize(skills, "skill"));
    }
    if agents > 0 {
        parts.push(pluralize(agents, "agent"));
    }
    if hooks > 0 {
        parts.push(pluralize(hooks, "hook"));
    }
    if mcp > 0 {
        parts.push(pluralize(mcp, "mcp server"));
    }
    parts.join(", ")
}

fn pluralize(count: usize, singular: &str) -> String {
    if count == 1 {
        format!("{count} {singular}")
    } else {
        format!("{count} {singular}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install_cache::{BackendDeployment, DeployedArtifactEntry};
    use std::collections::HashMap;

    fn make_entry(
        source: &str,
        version: &str,
        artifacts: Vec<(ArtifactKind, &str)>,
    ) -> PackageEntry {
        let mut deployed = HashMap::new();
        deployed.insert(
            "claude".to_string(),
            BackendDeployment {
                artifacts: artifacts
                    .into_iter()
                    .map(|(kind, name)| DeployedArtifactEntry {
                        artifact_type: kind,
                        name: name.to_string(),
                        deployed_path: format!("/deploy/{name}"),
                        deployed_hooks: vec![],
                        original_name: None,
                    })
                    .collect(),
                mcp_servers: vec![],
            },
        );
        PackageEntry {
            version: version.to_string(),
            source: source.to_string(),
            source_path: if source == "git" {
                "git@github.com:user/repo".to_string()
            } else {
                "/tmp/pkg".to_string()
            },
            integrity: "abc123".to_string(),
            archive_path: "/tmp/a.tar.gz".to_string(),
            deployed,
            resolved: if source == "git" {
                Some("abcdef1234567890".to_string())
            } else {
                None
            },
            tag: None,
        }
    }

    fn make_cache(packages: Vec<(&str, PackageEntry)>) -> InstallCache {
        InstallCache {
            version: 2,
            packages: packages
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        }
    }

    #[test]
    fn test_format_empty_project() {
        let cache = make_cache(vec![]);
        let output = format_package_list(&cache, false);
        assert_eq!(output, "No packages installed (project).\n");
    }

    #[test]
    fn test_format_empty_global() {
        let cache = make_cache(vec![]);
        let output = format_package_list(&cache, true);
        assert_eq!(output, "No packages installed (global).\n");
    }

    #[test]
    fn test_format_single_local_package() {
        let cache = make_cache(vec![(
            "@test/review",
            make_entry("local", "1.0.0", vec![(ArtifactKind::Skill, "review")]),
        )]);
        let output = format_package_list(&cache, false);
        assert!(output.contains("Installed packages (project):"));
        assert!(output.contains("@test/review"));
        assert!(output.contains("v1.0.0"));
        assert!(output.contains("[local]"));
        assert!(output.contains("1 skill"));
    }

    #[test]
    fn test_format_single_git_package() {
        let mut entry = make_entry("git", "2.0.0", vec![(ArtifactKind::Agent, "deploy")]);
        entry.tag = Some("v2.0.0".to_string());
        let cache = make_cache(vec![("@acme/deploy", entry)]);
        let output = format_package_list(&cache, true);
        assert!(output.contains("Installed packages (global):"));
        assert!(output.contains("@acme/deploy"));
        assert!(output.contains("v2.0.0"));
        assert!(output.contains("[git]"));
        assert!(output.contains("v2.0.0 @ abcdef1"));
        assert!(output.contains("1 agent"));
    }

    #[test]
    fn test_format_git_package_no_tag() {
        let entry = make_entry("git", "1.0.0", vec![(ArtifactKind::Skill, "lint")]);
        let cache = make_cache(vec![("@acme/lint", entry)]);
        let output = format_package_list(&cache, true);
        assert!(output.contains("[git]"));
        assert!(output.contains("@ abcdef1"));
        assert!(!output.contains("v1.0.0 @")); // no tag before @
    }

    #[test]
    fn test_format_mixed_sources() {
        let local = make_entry("local", "1.0.0", vec![(ArtifactKind::Skill, "review")]);
        let git = make_entry("git", "2.0.0", vec![(ArtifactKind::Agent, "deploy")]);
        let cache = make_cache(vec![("@acme/review", local), ("@acme/deploy", git)]);
        let output = format_package_list(&cache, true);
        assert!(output.contains("[local]"));
        assert!(output.contains("[git]"));
        // @acme/deploy should come before @acme/review (alphabetical)
        let pos_deploy = output.find("@acme/deploy").unwrap();
        let pos_review = output.find("@acme/review").unwrap();
        assert!(pos_deploy < pos_review);
    }

    #[test]
    fn test_format_multiple_artifact_types() {
        let entry = make_entry(
            "local",
            "1.0.0",
            vec![
                (ArtifactKind::Skill, "review"),
                (ArtifactKind::Skill, "lint"),
                (ArtifactKind::Agent, "deploy"),
                (ArtifactKind::Hook, "pre-check"),
            ],
        );
        let cache = make_cache(vec![("@test/multi", entry)]);
        let output = format_package_list(&cache, false);
        assert!(output.contains("2 skills"));
        assert!(output.contains("1 agent"));
        assert!(output.contains("1 hook"));
    }

    #[test]
    fn test_format_with_mcp_servers() {
        let mut entry = make_entry("local", "1.0.0", vec![]);
        entry
            .deployed
            .get_mut("claude")
            .unwrap()
            .mcp_servers = vec!["server-a".to_string(), "server-b".to_string()];
        let cache = make_cache(vec![("@test/mcp", entry)]);
        let output = format_package_list(&cache, true);
        assert!(output.contains("2 mcp servers"));
    }

    #[test]
    fn test_format_packages_sorted_alphabetically() {
        let a = make_entry("local", "1.0.0", vec![(ArtifactKind::Skill, "s")]);
        let b = make_entry("local", "1.0.0", vec![(ArtifactKind::Skill, "s")]);
        let c = make_entry("local", "1.0.0", vec![(ArtifactKind::Skill, "s")]);
        let cache = make_cache(vec![("@z/last", c), ("@a/first", a), ("@m/middle", b)]);
        let output = format_package_list(&cache, false);
        let pos_a = output.find("@a/first").unwrap();
        let pos_m = output.find("@m/middle").unwrap();
        let pos_z = output.find("@z/last").unwrap();
        assert!(pos_a < pos_m);
        assert!(pos_m < pos_z);
    }

    #[test]
    fn test_pluralize() {
        assert_eq!(pluralize(1, "skill"), "1 skill");
        assert_eq!(pluralize(2, "skill"), "2 skills");
        assert_eq!(pluralize(0, "skill"), "0 skills");
        assert_eq!(pluralize(1, "mcp server"), "1 mcp server");
        assert_eq!(pluralize(3, "mcp server"), "3 mcp servers");
    }

    #[test]
    fn test_format_package_no_artifacts() {
        let entry = make_entry("local", "1.0.0", vec![]);
        let cache = make_cache(vec![("@test/empty", entry)]);
        let output = format_package_list(&cache, false);
        assert!(output.contains("@test/empty"));
        assert!(output.contains("(no artifacts)"));
    }

    #[test]
    fn test_format_multi_backend_breakdown() {
        let mut deployed = HashMap::new();
        deployed.insert(
            "claude".to_string(),
            BackendDeployment {
                artifacts: vec![DeployedArtifactEntry {
                    artifact_type: ArtifactKind::Skill,
                    name: "review".to_string(),
                    deployed_path: "/deploy/review".to_string(),
                    deployed_hooks: vec![],
                    original_name: None,
                }],
                mcp_servers: vec![],
            },
        );
        deployed.insert(
            "agents".to_string(),
            BackendDeployment {
                artifacts: vec![DeployedArtifactEntry {
                    artifact_type: ArtifactKind::Skill,
                    name: "review".to_string(),
                    deployed_path: "/deploy/review".to_string(),
                    deployed_hooks: vec![],
                    original_name: None,
                }],
                mcp_servers: vec![],
            },
        );
        let entry = PackageEntry {
            version: "1.0.0".to_string(),
            source: "local".to_string(),
            source_path: "/tmp/pkg".to_string(),
            integrity: "abc".to_string(),
            archive_path: "/tmp/a.tar.gz".to_string(),
            deployed,
            resolved: None,
            tag: None,
        };
        let cache = make_cache(vec![("@test/multi-backend", entry)]);
        let output = format_package_list(&cache, false);
        assert!(output.contains("claude"));
        assert!(output.contains("agents"));
        assert!(output.contains("1 skill"));
    }
}
