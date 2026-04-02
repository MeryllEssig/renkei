use crate::artifact::{Artifact, ArtifactKind};
use crate::install_cache::InstallCache;

#[derive(Debug, Clone)]
pub struct Conflict {
    pub artifact_kind: ArtifactKind,
    pub artifact_name: String,
    pub owner_package: String,
}

/// Detect conflicts between incoming artifacts and already-deployed artifacts
/// owned by OTHER packages. Skips hooks (they merge into settings.json, no path collision).
pub fn detect_conflicts(
    artifacts: &[Artifact],
    install_cache: &InstallCache,
    current_package: &str,
) -> Vec<Conflict> {
    let mut conflicts = Vec::new();

    for art in artifacts {
        if art.kind == ArtifactKind::Hook {
            continue;
        }

        for (pkg_name, entry) in &install_cache.packages {
            if pkg_name == current_package {
                continue;
            }

            for deployed in entry.all_artifacts() {
                if deployed.artifact_type == art.kind && deployed.name == art.name {
                    conflicts.push(Conflict {
                        artifact_kind: art.kind.clone(),
                        artifact_name: art.name.clone(),
                        owner_package: pkg_name.clone(),
                    });
                }
            }
        }
    }

    conflicts.sort_by(|a, b| a.artifact_name.cmp(&b.artifact_name));
    conflicts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install_cache::{BackendDeployment, DeployedArtifactEntry, PackageEntry};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_artifact(kind: ArtifactKind, name: &str) -> Artifact {
        Artifact {
            kind,
            name: name.to_string(),
            source_path: PathBuf::from(format!("/tmp/{name}.md")),
        }
    }

    fn make_cache_entry(artifacts: Vec<(ArtifactKind, &str)>) -> PackageEntry {
        let mut deployed = HashMap::new();
        deployed.insert(
            "claude".to_string(),
            BackendDeployment {
                artifacts: artifacts
                    .into_iter()
                    .map(|(kind, name)| DeployedArtifactEntry {
                        artifact_type: kind,
                        name: name.to_string(),
                        deployed_path: format!("/p/{name}"),
                        deployed_hooks: vec![],
                        original_name: None,
                    })
                    .collect(),
                mcp_servers: vec![],
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
        }
    }

    #[test]
    fn test_no_conflict_empty_cache() {
        let artifacts = vec![make_artifact(ArtifactKind::Skill, "review")];
        let cache = InstallCache {
            version: 2,
            packages: HashMap::new(),
        };
        let conflicts = detect_conflicts(&artifacts, &cache, "@test/pkg-b");
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_no_conflict_same_package_reinstall() {
        let artifacts = vec![make_artifact(ArtifactKind::Skill, "review")];
        let mut packages = HashMap::new();
        packages.insert(
            "@test/pkg-a".to_string(),
            make_cache_entry(vec![(ArtifactKind::Skill, "review")]),
        );
        let cache = InstallCache {
            version: 2,
            packages,
        };
        // Installing the same package — no conflict
        let conflicts = detect_conflicts(&artifacts, &cache, "@test/pkg-a");
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_conflict_skill_different_package() {
        let artifacts = vec![make_artifact(ArtifactKind::Skill, "review")];
        let mut packages = HashMap::new();
        packages.insert(
            "@test/pkg-a".to_string(),
            make_cache_entry(vec![(ArtifactKind::Skill, "review")]),
        );
        let cache = InstallCache {
            version: 2,
            packages,
        };
        let conflicts = detect_conflicts(&artifacts, &cache, "@test/pkg-b");
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].artifact_name, "review");
        assert_eq!(conflicts[0].owner_package, "@test/pkg-a");
        assert_eq!(conflicts[0].artifact_kind, ArtifactKind::Skill);
    }

    #[test]
    fn test_conflict_agent_different_package() {
        let artifacts = vec![make_artifact(ArtifactKind::Agent, "deploy")];
        let mut packages = HashMap::new();
        packages.insert(
            "@test/pkg-a".to_string(),
            make_cache_entry(vec![(ArtifactKind::Agent, "deploy")]),
        );
        let cache = InstallCache {
            version: 2,
            packages,
        };
        let conflicts = detect_conflicts(&artifacts, &cache, "@test/pkg-b");
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].artifact_kind, ArtifactKind::Agent);
    }

    #[test]
    fn test_no_conflict_different_kinds() {
        // Skill "review" and Agent "review" deploy to different directories — no conflict
        let artifacts = vec![make_artifact(ArtifactKind::Skill, "review")];
        let mut packages = HashMap::new();
        packages.insert(
            "@test/pkg-a".to_string(),
            make_cache_entry(vec![(ArtifactKind::Agent, "review")]),
        );
        let cache = InstallCache {
            version: 2,
            packages,
        };
        let conflicts = detect_conflicts(&artifacts, &cache, "@test/pkg-b");
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_hooks_never_conflict() {
        let artifacts = vec![make_artifact(ArtifactKind::Hook, "lint")];
        let mut packages = HashMap::new();
        packages.insert(
            "@test/pkg-a".to_string(),
            make_cache_entry(vec![(ArtifactKind::Hook, "lint")]),
        );
        let cache = InstallCache {
            version: 2,
            packages,
        };
        let conflicts = detect_conflicts(&artifacts, &cache, "@test/pkg-b");
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_multiple_conflicts() {
        let artifacts = vec![
            make_artifact(ArtifactKind::Skill, "lint"),
            make_artifact(ArtifactKind::Skill, "review"),
        ];
        let mut packages = HashMap::new();
        packages.insert(
            "@test/pkg-a".to_string(),
            make_cache_entry(vec![
                (ArtifactKind::Skill, "review"),
                (ArtifactKind::Skill, "lint"),
            ]),
        );
        let cache = InstallCache {
            version: 2,
            packages,
        };
        let conflicts = detect_conflicts(&artifacts, &cache, "@test/pkg-b");
        assert_eq!(conflicts.len(), 2);
        // Sorted alphabetically
        assert_eq!(conflicts[0].artifact_name, "lint");
        assert_eq!(conflicts[1].artifact_name, "review");
    }

    #[test]
    fn test_no_conflict_different_names() {
        let artifacts = vec![make_artifact(ArtifactKind::Skill, "lint")];
        let mut packages = HashMap::new();
        packages.insert(
            "@test/pkg-a".to_string(),
            make_cache_entry(vec![(ArtifactKind::Skill, "review")]),
        );
        let cache = InstallCache {
            version: 2,
            packages,
        };
        let conflicts = detect_conflicts(&artifacts, &cache, "@test/pkg-b");
        assert!(conflicts.is_empty());
    }
}
