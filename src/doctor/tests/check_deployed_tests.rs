use crate::artifact::ArtifactKind;
use crate::doctor::checks;
use crate::doctor::types::DiagnosticKind;
use crate::hook::DeployedHookEntry;
use crate::install_cache::DeployedArtifactEntry;
use tempfile::tempdir;

use super::{make_artifact, make_entry};

#[test]
fn test_deployed_files_all_exist() {
    let dir = tempdir().unwrap();
    let skill_dir = dir.path().join("review");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "# Skill").unwrap();
    let agent_path = dir.path().join("agent.md");
    std::fs::write(&agent_path, "# Agent").unwrap();

    let entry = make_entry(vec![
        make_artifact(ArtifactKind::Skill, "review", skill_dir.to_str().unwrap()),
        make_artifact(ArtifactKind::Agent, "deploy", agent_path.to_str().unwrap()),
    ]);
    assert!(checks::check_deployed_files(&entry).is_empty());
}

#[test]
fn test_deployed_files_missing() {
    let entry = make_entry(vec![make_artifact(
        ArtifactKind::Skill,
        "review",
        "/nonexistent/path/SKILL.md",
    )]);
    let issues = checks::check_deployed_files(&entry);
    assert_eq!(issues.len(), 1);
    assert!(
        matches!(&issues[0], DiagnosticKind::FileMissing { artifact_name, .. } if artifact_name == "review")
    );
}

#[test]
fn test_deployed_files_hook_skipped() {
    let entry = make_entry(vec![DeployedArtifactEntry {
        artifact_type: ArtifactKind::Hook,
        name: "lint".to_string(),
        deployed_path: "/nonexistent/settings.json".to_string(),
        deployed_hooks: vec![DeployedHookEntry {
            event: "PreToolUse".to_string(),
            matcher: Some("bash".to_string()),
            command: "lint.sh".to_string(),
        }],
        original_name: None,
    }]);
    assert!(checks::check_deployed_files(&entry).is_empty());
}

#[test]
fn test_deployed_files_multiple_missing() {
    let entry = make_entry(vec![
        make_artifact(ArtifactKind::Skill, "a", "/missing/a"),
        make_artifact(ArtifactKind::Agent, "b", "/missing/b"),
    ]);
    let issues = checks::check_deployed_files(&entry);
    assert_eq!(issues.len(), 2);
}
