use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_migrate_flat_skills() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("review.md"),
        "---\nname: review\n---\nReview code changes",
    )
    .unwrap();
    fs::write(
        dir.path().join("lint.md"),
        "---\nname: lint\n---\nLint the code",
    )
    .unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .arg("migrate")
        .arg(dir.path().to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("Migrated successfully"));

    assert!(dir.path().join("renkei.json").exists());
    assert!(dir.path().join("skills/review/SKILL.md").exists());
    assert!(dir.path().join("skills/lint/SKILL.md").exists());
}

#[test]
fn test_migrate_hooks() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("pre-check.json"),
        r#"[{"event": "before_tool", "command": "echo check"}]"#,
    )
    .unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .arg("migrate")
        .arg(dir.path().to_str().unwrap())
        .assert()
        .success();

    assert!(dir.path().join("hooks/pre-check.json").exists());
    assert!(dir.path().join("renkei.json").exists());
}

#[test]
fn test_migrate_already_renkei_package() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("renkei.json"),
        r#"{"name": "@test/pkg", "version": "1.0.0"}"#,
    )
    .unwrap();
    fs::write(dir.path().join("review.md"), "content").unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .arg("migrate")
        .arg(dir.path().to_str().unwrap())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Already a Renkei package"));
}

#[test]
fn test_migrate_empty_dir() {
    let dir = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .arg("migrate")
        .arg(dir.path().to_str().unwrap())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Nothing to migrate"));
}

#[test]
fn test_migrate_pre_organized() {
    let dir = tempdir().unwrap();
    let skill_dir = dir.path().join("skills/review");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), "# Review skill").unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .arg("migrate")
        .arg(dir.path().to_str().unwrap())
        .assert()
        .success();

    // File stays in place
    assert!(dir.path().join("skills/review/SKILL.md").exists());
    assert!(dir.path().join("renkei.json").exists());

    // Manifest is valid
    let content = fs::read_to_string(dir.path().join("renkei.json")).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(manifest["name"].as_str().unwrap().starts_with("@migrated/"));
}

#[test]
fn test_migrate_then_package() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("review.md"),
        "---\nname: review\n---\nReview content",
    )
    .unwrap();

    // Migrate
    Command::cargo_bin("rk")
        .unwrap()
        .arg("migrate")
        .arg(dir.path().to_str().unwrap())
        .assert()
        .success();

    // Package should succeed on the migrated directory
    Command::cargo_bin("rk")
        .unwrap()
        .current_dir(dir.path())
        .arg("package")
        .assert()
        .success();
}

#[test]
fn test_migrate_mixed_content() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("review.md"),
        "---\nname: review\n---\nSkill content",
    )
    .unwrap();
    fs::write(
        dir.path().join("hook.json"),
        r#"[{"event": "after_tool", "command": "echo done"}]"#,
    )
    .unwrap();
    fs::create_dir_all(dir.path().join("agents")).unwrap();
    fs::write(dir.path().join("agents/helper.md"), "# Agent helper").unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .arg("migrate")
        .arg(dir.path().to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("1 skill(s)"))
        .stdout(predicate::str::contains("1 hook(s)"))
        .stdout(predicate::str::contains("1 agent(s)"));

    assert!(dir.path().join("skills/review/SKILL.md").exists());
    assert!(dir.path().join("hooks/hook.json").exists());
    assert!(dir.path().join("agents/helper.md").exists());
}
