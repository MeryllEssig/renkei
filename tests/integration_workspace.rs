use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn fixture_path(name: &str) -> String {
    let base = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures")
        .join(name);
    base.to_string_lossy().to_string()
}

fn setup_claude_home(home: &Path) {
    fs::create_dir_all(home.join(".claude")).unwrap();
}

#[test]
fn test_install_workspace_installs_all_members() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("workspace-pkg"))
        .assert()
        .success()
        .stdout(predicate::str::contains("workspace"))
        .stdout(predicate::str::contains("@test/member-a"))
        .stdout(predicate::str::contains("@test/member-b"));

    // Verify both skills deployed
    let review = home.path().join(".claude/skills/review/SKILL.md");
    let lint = home.path().join(".claude/skills/lint/SKILL.md");
    assert!(review.exists(), "review skill should be deployed");
    assert!(lint.exists(), "lint skill should be deployed");

    // Verify install-cache has both members
    let cache_path = home.path().join(".renkei/install-cache.json");
    let content = fs::read_to_string(&cache_path).unwrap();
    let cache: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(cache["packages"]["@test/member-a"].is_object());
    assert!(cache["packages"]["@test/member-b"].is_object());
}

#[test]
fn test_workspace_members_in_list() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    // Install workspace
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("workspace-pkg"))
        .assert()
        .success();

    // List should show both members
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("list")
        .arg("-g")
        .assert()
        .success()
        .stdout(predicate::str::contains("@test/member-a"))
        .stdout(predicate::str::contains("@test/member-b"));
}

#[test]
fn test_workspace_members_in_lockfile() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("workspace-pkg"))
        .assert()
        .success();

    // Verify lockfile has both members
    let lockfile_path = home.path().join(".renkei/rk.lock");
    assert!(lockfile_path.exists(), "Global lockfile should be created");

    let content = fs::read_to_string(&lockfile_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["lockfileVersion"], 1);
    assert!(parsed["packages"]["@test/member-a"].is_object());
    assert!(parsed["packages"]["@test/member-b"].is_object());

    // Verify integrity fields
    let integrity_a = parsed["packages"]["@test/member-a"]["integrity"]
        .as_str()
        .unwrap();
    let integrity_b = parsed["packages"]["@test/member-b"]["integrity"]
        .as_str()
        .unwrap();
    assert!(integrity_a.starts_with("sha256-"));
    assert!(integrity_b.starts_with("sha256-"));
}

#[test]
fn test_install_no_args_workspace_without_lockfile() {
    let home = tempdir().unwrap();

    // Create a git repo that has a workspace renkei.json but no lockfile
    let project = tempdir().unwrap();
    std::process::Command::new("git")
        .args(["init"])
        .arg(project.path())
        .output()
        .unwrap();
    fs::write(
        project.path().join("renkei.json"),
        r#"{ "workspace": ["member-a"] }"#,
    )
    .unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(project.path())
        .arg("install")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Workspace detected"))
        .stderr(predicate::str::contains("rk install --link ."));
}

#[test]
fn test_install_workspace_missing_member_manifest() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("workspace-broken"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("Manifest not found"));

    // Verify no members were partially installed
    assert!(
        !home.path().join(".claude/skills/foo/SKILL.md").exists(),
        "No member should be installed on failure"
    );
}

#[test]
fn test_workspace_lockfile_roundtrip() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    // Step 1: Install workspace
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("workspace-pkg"))
        .assert()
        .success();

    // Step 2: Delete deployed skills (keep archives + lockfile)
    fs::remove_dir_all(home.path().join(".claude/skills")).unwrap();
    assert!(!home.path().join(".claude/skills/review/SKILL.md").exists());

    // Step 3: Restore from lockfile
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .assert()
        .success()
        .stdout(predicate::str::contains("Restoring"));

    // Step 4: Verify both skills are re-deployed
    assert!(home.path().join(".claude/skills/review/SKILL.md").exists());
    assert!(home.path().join(".claude/skills/lint/SKILL.md").exists());
}
