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

fn init_git_repo(path: &Path) {
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(path)
        .output()
        .expect("git init failed");
}

#[test]
fn test_uninstall_global_after_install() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    // Install globally
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["install", "-g", &fixture_path("valid-package")])
        .assert()
        .success();

    // Verify skill exists
    let skill_path = home.path().join(".claude/skills/renkei-review/SKILL.md");
    assert!(skill_path.exists());

    // Uninstall
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["uninstall", "-g", "@test/sample-workflow"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Uninstalled"));

    // Verify skill removed
    assert!(!skill_path.exists());

    // Verify install-cache is empty
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["list", "-g"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No packages installed"));
}

#[test]
fn test_uninstall_project_after_install() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    setup_claude_home(home.path());
    init_git_repo(project.path());

    // Install in project scope
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(project.path())
        .args(["install", &fixture_path("valid-package")])
        .assert()
        .success();

    // On macOS tempdir resolves through /private, canonicalize to match
    let canonical_project = project.path().canonicalize().unwrap();
    let skill_path = canonical_project.join(".claude/skills/renkei-review/SKILL.md");
    assert!(skill_path.exists());

    // Uninstall
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(project.path())
        .args(["uninstall", "@test/sample-workflow"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Uninstalled"));

    // Verify skill removed
    assert!(!skill_path.exists());

    // Verify install-cache empty
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(project.path())
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No packages installed"));
}

#[test]
fn test_uninstall_not_found_global() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["uninstall", "-g", "@nonexistent/pkg"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not installed"))
        .stderr(predicate::str::contains("global"));
}

#[test]
fn test_uninstall_not_found_project() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    setup_claude_home(home.path());
    init_git_repo(project.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(project.path())
        .args(["uninstall", "@nonexistent/pkg"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not installed"))
        .stderr(predicate::str::contains("project"));
}

#[test]
fn test_uninstall_leaves_other_packages() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    // Install two packages globally (no conflicting skill names)
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["install", "-g", &fixture_path("valid-package")])
        .assert()
        .success();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["install", "-g", &fixture_path("env-only")])
        .assert()
        .success();

    // Verify both skills exist
    assert!(home
        .path()
        .join(".claude/skills/renkei-review/SKILL.md")
        .exists());
    assert!(home
        .path()
        .join(".claude/skills/renkei-check/SKILL.md")
        .exists());

    // Uninstall only the first one
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["uninstall", "-g", "@test/sample-workflow"])
        .assert()
        .success();

    // The other package should still be listed and its skill untouched
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["list", "-g"])
        .assert()
        .success()
        .stdout(predicate::str::contains("@test/env-only"))
        .stdout(predicate::str::contains("@test/sample-workflow").not());

    assert!(!home
        .path()
        .join(".claude/skills/renkei-review/SKILL.md")
        .exists());
    assert!(home
        .path()
        .join(".claude/skills/renkei-check/SKILL.md")
        .exists());
}

#[test]
fn test_uninstall_outside_git_repo() {
    let home = tempdir().unwrap();
    let no_git = tempdir().unwrap();
    setup_claude_home(home.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(no_git.path())
        .env("GIT_CEILING_DIRECTORIES", no_git.path())
        .args(["uninstall", "@test/pkg"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No project root"));
}
