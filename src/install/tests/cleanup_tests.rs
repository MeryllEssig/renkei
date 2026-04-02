use std::collections::HashMap;
use std::fs;

use tempfile::tempdir;

use crate::artifact::ArtifactKind;
use crate::backend::DeployedArtifact;
use crate::config::Config;
use crate::install::cleanup;
use crate::install_cache::InstallCache;

use super::helpers::make_cache_with_artifacts;

#[test]
fn test_cleanup_removes_old_artifacts() {
    let home = tempdir().unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());
    let dir = tempdir().unwrap();
    let skill_dir = dir.path().join("renkei-review");
    fs::create_dir_all(&skill_dir).unwrap();
    let file1 = skill_dir.join("SKILL.md");
    let file2 = dir.path().join("agent.md");
    fs::write(&file1, "old skill").unwrap();
    fs::write(&file2, "old agent").unwrap();

    let cache = make_cache_with_artifacts(vec![
        (ArtifactKind::Skill, "review", file1.to_str().unwrap()),
        (ArtifactKind::Agent, "deploy", file2.to_str().unwrap()),
    ]);

    cleanup::cleanup_previous_installation("@test/pkg", &cache, &config);
    assert!(!file1.exists());
    assert!(!file2.exists());
    assert!(!skill_dir.exists());
}

#[test]
fn test_cleanup_noop_on_missing_package() {
    let home = tempdir().unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());
    let cache = InstallCache {
        version: 1,
        packages: HashMap::new(),
    };
    cleanup::cleanup_previous_installation("@test/nonexistent", &cache, &config);
}

#[test]
fn test_cleanup_tolerates_already_missing_file() {
    let home = tempdir().unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());
    let cache = make_cache_with_artifacts(vec![(
        ArtifactKind::Skill,
        "gone",
        "/tmp/nonexistent/SKILL.md",
    )]);
    cleanup::cleanup_previous_installation("@test/pkg", &cache, &config);
}

#[test]
fn test_rollback_removes_deployed_files() {
    let home = tempdir().unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());
    let dir = tempdir().unwrap();
    let file1 = dir.path().join("file1.md");
    let file2 = dir.path().join("file2.md");
    fs::write(&file1, "content1").unwrap();
    fs::write(&file2, "content2").unwrap();

    let deployed = vec![
        DeployedArtifact {
            artifact_kind: ArtifactKind::Skill,
            artifact_name: "s1".to_string(),
            deployed_path: file1.clone(),
            deployed_hooks: vec![],
        },
        DeployedArtifact {
            artifact_kind: ArtifactKind::Skill,
            artifact_name: "s2".to_string(),
            deployed_path: file2.clone(),
            deployed_hooks: vec![],
        },
    ];

    cleanup::rollback(&deployed, &config);
    assert!(!file1.exists());
    assert!(!file2.exists());
}

#[test]
fn test_rollback_removes_empty_parent_dir() {
    let home = tempdir().unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());
    let dir = tempdir().unwrap();
    let skill_dir = dir.path().join("renkei-review");
    fs::create_dir_all(&skill_dir).unwrap();
    let file = skill_dir.join("SKILL.md");
    fs::write(&file, "content").unwrap();

    let deployed = vec![DeployedArtifact {
        artifact_kind: ArtifactKind::Skill,
        artifact_name: "review".to_string(),
        deployed_path: file.clone(),
        deployed_hooks: vec![],
    }];

    cleanup::rollback(&deployed, &config);
    assert!(!file.exists());
    assert!(!skill_dir.exists());
}

#[test]
fn test_rollback_skips_missing_files() {
    let home = tempdir().unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());
    let dir = tempdir().unwrap();
    let missing = dir.path().join("nonexistent.md");

    let deployed = vec![DeployedArtifact {
        artifact_kind: ArtifactKind::Skill,
        artifact_name: "ghost".to_string(),
        deployed_path: missing,
        deployed_hooks: vec![],
    }];

    cleanup::rollback(&deployed, &config);
}
