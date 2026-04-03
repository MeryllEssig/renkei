use crate::artifact::ArtifactKind;
use crate::cache;
use crate::config::Config;
use crate::doctor::checks;
use crate::doctor::types::{ArchiveState, DiagnosticKind};
use crate::install_cache::DeployedArtifactEntry;
use crate::manifest::{ManifestScope, ValidatedManifest};
use semver::Version;
use tempfile::tempdir;

use super::{make_artifact, make_entry};

fn make_test_manifest() -> ValidatedManifest {
    ValidatedManifest {
        scope: "test".to_string(),
        short_name: "sample".to_string(),
        full_name: "@test/sample".to_string(),
        version: Version::new(0, 1, 0),
        install_scope: ManifestScope::Any,
        description: "test".to_string(),
        author: "tester".to_string(),
        license: "MIT".to_string(),
        backends: vec!["claude".to_string()],
    }
}

fn setup_package_with_skill(dir: &std::path::Path, skill_name: &str, content: &str) {
    std::fs::write(
        dir.join("renkei.json"),
        r#"{"name":"@test/sample","version":"0.1.0","description":"test","author":"tester","license":"MIT","backends":["claude"]}"#,
    ).unwrap();
    let skill_dir = dir.join("skills").join(skill_name);
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), content).unwrap();
}

#[test]
fn test_skill_unmodified() {
    let home = tempdir().unwrap();
    let pkg = tempdir().unwrap();
    let deploy = tempdir().unwrap();

    setup_package_with_skill(pkg.path(), "review", "# Review skill");
    let config = Config::with_home_dir(home.path().to_path_buf());
    let manifest = make_test_manifest();
    let (archive_path, _) = cache::create_archive(pkg.path(), &manifest, &config).unwrap();

    let deployed_dir = deploy.path().join("renkei-review");
    std::fs::create_dir_all(&deployed_dir).unwrap();
    std::fs::write(deployed_dir.join("SKILL.md"), "# Review skill").unwrap();

    let mut entry = make_entry(vec![make_artifact(
        ArtifactKind::Skill,
        "review",
        deployed_dir.to_str().unwrap(),
    )]);
    entry.archive_path = archive_path.to_string_lossy().to_string();

    assert!(checks::check_skill_modifications(&entry).is_empty());
}

#[test]
fn test_skill_modified() {
    let home = tempdir().unwrap();
    let pkg = tempdir().unwrap();
    let deploy = tempdir().unwrap();

    setup_package_with_skill(pkg.path(), "review", "# Review skill");
    let config = Config::with_home_dir(home.path().to_path_buf());
    let manifest = make_test_manifest();
    let (archive_path, _) = cache::create_archive(pkg.path(), &manifest, &config).unwrap();

    let deployed_dir = deploy.path().join("renkei-review");
    std::fs::create_dir_all(&deployed_dir).unwrap();
    std::fs::write(deployed_dir.join("SKILL.md"), "# Modified review skill").unwrap();

    let mut entry = make_entry(vec![make_artifact(
        ArtifactKind::Skill,
        "review",
        deployed_dir.to_str().unwrap(),
    )]);
    entry.archive_path = archive_path.to_string_lossy().to_string();

    let issues = checks::check_skill_modifications(&entry);
    assert_eq!(issues.len(), 1);
    assert!(
        matches!(&issues[0], DiagnosticKind::SkillModified { artifact_name, .. } if artifact_name == "review")
    );
}

#[test]
fn test_skill_modification_missing_file_skipped() {
    let entry = make_entry(vec![make_artifact(
        ArtifactKind::Skill,
        "review",
        "/nonexistent/renkei-review",
    )]);
    assert!(checks::check_skill_modifications(&entry).is_empty());
}

#[test]
fn test_skill_modification_missing_archive_skips() {
    let deploy = tempdir().unwrap();
    let deployed_dir = deploy.path().join("renkei-review");
    std::fs::create_dir_all(&deployed_dir).unwrap();
    std::fs::write(deployed_dir.join("SKILL.md"), "# Skill").unwrap();

    let mut entry = make_entry(vec![make_artifact(
        ArtifactKind::Skill,
        "review",
        deployed_dir.to_str().unwrap(),
    )]);
    entry.archive_path = "/nonexistent/archive.tar.gz".to_string();

    assert!(checks::check_skill_modifications(&entry).is_empty());
}

#[test]
fn test_check_archive_available() {
    let dir = tempdir().unwrap();
    let archive = dir.path().join("archive.tar.gz");
    std::fs::write(&archive, "fake").unwrap();

    let mut entry = make_entry(vec![]);
    entry.archive_path = archive.to_string_lossy().to_string();
    assert!(matches!(
        checks::check_archive(&entry),
        ArchiveState::Available
    ));
}

#[test]
fn test_check_archive_missing() {
    let mut entry = make_entry(vec![]);
    entry.archive_path = "/nonexistent/archive.tar.gz".to_string();
    assert!(matches!(
        checks::check_archive(&entry),
        ArchiveState::Missing(_)
    ));
}

#[test]
fn test_skill_modification_uses_original_name() {
    let home = tempdir().unwrap();
    let pkg = tempdir().unwrap();
    let deploy = tempdir().unwrap();

    setup_package_with_skill(pkg.path(), "review", "# Review skill");
    let config = Config::with_home_dir(home.path().to_path_buf());
    let manifest = make_test_manifest();
    let (archive_path, _) = cache::create_archive(pkg.path(), &manifest, &config).unwrap();

    let deployed_dir = deploy.path().join("renkei-review-v2");
    std::fs::create_dir_all(&deployed_dir).unwrap();
    std::fs::write(deployed_dir.join("SKILL.md"), "# Review skill").unwrap();

    let mut entry = make_entry(vec![DeployedArtifactEntry {
        artifact_type: ArtifactKind::Skill,
        name: "review-v2".to_string(),
        deployed_path: deployed_dir.to_string_lossy().to_string(),
        deployed_hooks: vec![],
        original_name: Some("review".to_string()),
    }]);
    entry.archive_path = archive_path.to_string_lossy().to_string();

    assert!(checks::check_skill_modifications(&entry).is_empty());
}

#[test]
fn test_skill_modification_agent_skipped() {
    let mut entry = make_entry(vec![make_artifact(
        ArtifactKind::Agent,
        "deploy",
        "/nonexistent/agent.md",
    )]);
    entry.archive_path = "/nonexistent/archive.tar.gz".to_string();
    assert!(checks::check_skill_modifications(&entry).is_empty());
}
