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
fn test_install_creates_lockfile_global() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("valid-package"))
        .assert()
        .success();

    // Verify lockfile exists
    let lockfile_path = home.path().join(".renkei/rk.lock");
    assert!(lockfile_path.exists(), "Global lockfile should be created");

    // Verify lockfile content
    let content = fs::read_to_string(&lockfile_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["lockfileVersion"], 1);
    assert!(parsed["packages"]["@test/sample-workflow"].is_object());
    assert_eq!(
        parsed["packages"]["@test/sample-workflow"]["version"],
        "0.1.0"
    );
    let integrity = parsed["packages"]["@test/sample-workflow"]["integrity"]
        .as_str()
        .unwrap();
    assert!(
        integrity.starts_with("sha256-"),
        "Integrity should have sha256- prefix"
    );
}

#[test]
fn test_install_no_args_without_lockfile_fails_global() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No lockfile found"));
}

#[test]
fn test_install_no_args_without_lockfile_fails_project() {
    let home = tempdir().unwrap();

    // Create a git repo for project root detection
    let project = tempdir().unwrap();
    std::process::Command::new("git")
        .args(["init"])
        .arg(project.path())
        .output()
        .unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(project.path())
        .arg("install")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No lockfile found"));
}

#[test]
fn test_lockfile_roundtrip_global() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    // Step 1: install a package
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("valid-package"))
        .assert()
        .success();

    // Verify skill is deployed
    let skill_path = home.path().join(".claude/skills/review/SKILL.md");
    assert!(skill_path.exists());

    // Step 2: delete deployed files (but keep archive and lockfile)
    fs::remove_dir_all(home.path().join(".claude/skills")).unwrap();
    assert!(!skill_path.exists());

    // Also clear install-cache to simulate a clean state
    fs::remove_file(home.path().join(".renkei/install-cache.json")).unwrap();

    // Step 3: install from lockfile (no args)
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .assert()
        .success()
        .stdout(predicate::str::contains("Restoring"));

    // Verify skill is re-deployed
    assert!(
        skill_path.exists(),
        "Skill should be re-deployed from lockfile"
    );
}

#[test]
fn test_uninstall_removes_lockfile_entry() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    // Install a package
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("valid-package"))
        .assert()
        .success();

    // Verify lockfile has the entry
    let lockfile_path = home.path().join(".renkei/rk.lock");
    let content = fs::read_to_string(&lockfile_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed["packages"]["@test/sample-workflow"].is_object());

    // Uninstall
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("uninstall")
        .arg("-g")
        .arg("@test/sample-workflow")
        .assert()
        .success();

    // Verify lockfile no longer has the entry
    let content = fs::read_to_string(&lockfile_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(
        parsed["packages"]["@test/sample-workflow"].is_null(),
        "Package should be removed from lockfile after uninstall"
    );
}
