use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn fixture_path(name: &str) -> String {
    std::env::current_dir()
        .unwrap()
        .join("tests/fixtures")
        .join(name)
        .to_string_lossy()
        .to_string()
}

fn setup_claude_home(home: &Path) {
    fs::create_dir_all(home.join(".claude")).unwrap();
}

#[test]
fn test_config_set_get_roundtrip() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["config", "set", "defaults.backends", "claude,cursor"])
        .assert()
        .success();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["config", "get", "defaults.backends"])
        .assert()
        .success()
        .stdout(predicate::str::contains("claude"))
        .stdout(predicate::str::contains("cursor"));
}

#[test]
fn test_config_list_output() {
    let home = tempdir().unwrap();

    // List on empty config returns defaults JSON
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["config", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("defaults"));

    // After setting, list shows the value
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["config", "set", "defaults.backends", "claude"])
        .assert()
        .success();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["config", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("claude"));
}

#[test]
fn test_config_set_invalid_backend_fails() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["config", "set", "defaults.backends", "not-a-backend"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not-a-backend"));
}

#[test]
fn test_config_set_invalid_key_fails() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["config", "set", "unknown.key", "value"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown.key"));
}

#[test]
#[ignore = "Phase 2 rework: assertions assume pre-opt-in resolver"]
fn test_install_uses_config_backends() {
    // Set config to only use agents, then install a multi-backend package.
    // Only agents backend should be used.
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    // Save user config: only use "agents" backend
    let renkei_dir = home.path().join(".renkei");
    fs::create_dir_all(&renkei_dir).unwrap();
    fs::write(
        renkei_dir.join("config.json"),
        r#"{"defaults":{"backends":["agents"]}}"#,
    )
    .unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("multi-backend-package"))
        .assert()
        .success()
        .stdout(predicate::str::contains("Done."));

    // Agents skill exists
    assert!(
        home.path().join(".agents/skills/review/SKILL.md").exists(),
        "Agents skill should be deployed"
    );

    // Claude skill should NOT exist (not in config backends)
    assert!(
        !home.path().join(".claude/skills/review/SKILL.md").exists(),
        "Claude skill should not be deployed when config only has agents"
    );
}

#[test]
#[ignore = "Phase 2 rework: assertions assume pre-opt-in resolver"]
fn test_install_falls_back_to_autodetect_without_config() {
    // No user config → auto-detect: only .claude is present → only claude backend used
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    // No .renkei/config.json

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("multi-backend-package"))
        .assert()
        .success();

    // Claude installed (detected)
    assert!(
        home.path().join(".claude/skills/review/SKILL.md").exists(),
        "Claude skill should be deployed via auto-detect"
    );
    // Agents also installed (always detected)
    assert!(
        home.path().join(".agents/skills/review/SKILL.md").exists(),
        "Agents skill should be deployed (always detected)"
    );
}
