use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

fn fixture_path(name: &str) -> String {
    let base = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures")
        .join(name);
    base.to_string_lossy().to_string()
}

#[test]
fn test_install_valid_local_package() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg(fixture_path("valid-package"))
        .assert()
        .success()
        .stdout(predicate::str::contains("Done."));

    // Verify skill deployed
    let skill_path = home.path().join(".claude/skills/renkei-review/SKILL.md");
    assert!(
        skill_path.exists(),
        "SKILL.md should exist at {:?}",
        skill_path
    );

    // Verify content matches source
    let deployed = fs::read_to_string(&skill_path).unwrap();
    let source = fs::read_to_string(
        std::env::current_dir()
            .unwrap()
            .join("tests/fixtures/valid-package/skills/review.md"),
    )
    .unwrap();
    assert_eq!(deployed, source);

    // Verify archive exists
    let archive = home
        .path()
        .join(".renkei/cache/@test/sample-workflow/0.1.0.tar.gz");
    assert!(archive.exists(), "Archive should exist at {:?}", archive);

    // Verify install-cache.json
    let cache_path = home.path().join(".renkei/install-cache.json");
    assert!(cache_path.exists());
    let cache_content = fs::read_to_string(&cache_path).unwrap();
    let cache: serde_json::Value = serde_json::from_str(&cache_content).unwrap();
    assert!(cache["packages"]["@test/sample-workflow"].is_object());
    assert_eq!(
        cache["packages"]["@test/sample-workflow"]["version"],
        "0.1.0"
    );
    let artifacts = &cache["packages"]["@test/sample-workflow"]["deployed_artifacts"];
    assert_eq!(artifacts.as_array().unwrap().len(), 1);
    assert_eq!(artifacts[0]["name"], "review");
}

#[test]
fn test_install_multi_skill_package() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg(fixture_path("multi-skill-package"))
        .assert()
        .success()
        .stdout(predicate::str::contains("2 artifact(s)"));

    assert!(home
        .path()
        .join(".claude/skills/renkei-review/SKILL.md")
        .exists());
    assert!(home
        .path()
        .join(".claude/skills/renkei-lint/SKILL.md")
        .exists());
}

#[test]
fn test_install_missing_name_fails() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg(fixture_path("missing-name"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("name"));
}

#[test]
fn test_install_bad_scope_fails() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg(fixture_path("bad-scope"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("@scope/name"));
}

#[test]
fn test_install_bad_semver_fails() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg(fixture_path("bad-semver"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("not.a.version"));
}

#[test]
fn test_install_missing_fields_fails() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg(fixture_path("missing-fields"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing field"));
}

#[test]
fn test_install_no_skills_fails() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg(fixture_path("no-skills-package"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("No artifacts found"));
}

#[test]
fn test_install_nonexistent_path() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("/nonexistent/path/to/nowhere")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Manifest not found"));
}

#[test]
fn test_install_mixed_package() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg(fixture_path("mixed-package"))
        .assert()
        .success()
        .stdout(predicate::str::contains("3 artifact(s)"));

    // Verify skills deployed
    assert!(home
        .path()
        .join(".claude/skills/renkei-review/SKILL.md")
        .exists());
    assert!(home
        .path()
        .join(".claude/skills/renkei-lint/SKILL.md")
        .exists());

    // Verify agent deployed (no renkei- prefix, no subdirectory)
    assert!(home.path().join(".claude/agents/deploy.md").exists());

    // Verify install-cache.json
    let cache_path = home.path().join(".renkei/install-cache.json");
    let cache_content = fs::read_to_string(&cache_path).unwrap();
    let cache: serde_json::Value = serde_json::from_str(&cache_content).unwrap();
    let artifacts = &cache["packages"]["@test/mixed"]["deployed_artifacts"];
    assert_eq!(artifacts.as_array().unwrap().len(), 3);
}

#[test]
fn test_install_agent_deploy_path() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg(fixture_path("mixed-package"))
        .assert()
        .success();

    let deployed = fs::read_to_string(home.path().join(".claude/agents/deploy.md")).unwrap();
    let source = fs::read_to_string(
        std::env::current_dir()
            .unwrap()
            .join("tests/fixtures/mixed-package/agents/deploy.md"),
    )
    .unwrap();
    assert_eq!(deployed, source);
}

#[test]
fn test_reinstall_replaces_artifacts() {
    let home = tempdir().unwrap();

    // First install
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg(fixture_path("mixed-package"))
        .assert()
        .success();

    // Second install (reinstall)
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg(fixture_path("mixed-package"))
        .assert()
        .success()
        .stdout(predicate::str::contains("3 artifact(s)"));

    // Verify still exactly 1 cache entry with 3 artifacts
    let cache_path = home.path().join(".renkei/install-cache.json");
    let cache_content = fs::read_to_string(&cache_path).unwrap();
    let cache: serde_json::Value = serde_json::from_str(&cache_content).unwrap();
    let packages = cache["packages"].as_object().unwrap();
    assert_eq!(packages.len(), 1);
    let artifacts = &packages["@test/mixed"]["deployed_artifacts"];
    assert_eq!(artifacts.as_array().unwrap().len(), 3);

    // Verify all files still exist after reinstall
    assert!(home
        .path()
        .join(".claude/skills/renkei-review/SKILL.md")
        .exists());
    assert!(home
        .path()
        .join(".claude/skills/renkei-lint/SKILL.md")
        .exists());
    assert!(home.path().join(".claude/agents/deploy.md").exists());
}

#[test]
fn test_reinstall_updates_cache() {
    let home = tempdir().unwrap();

    // Install valid-package first
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg(fixture_path("valid-package"))
        .assert()
        .success();

    // Reinstall same package
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg(fixture_path("valid-package"))
        .assert()
        .success();

    // Cache should have exactly 1 entry
    let cache_path = home.path().join(".renkei/install-cache.json");
    let cache_content = fs::read_to_string(&cache_path).unwrap();
    let cache: serde_json::Value = serde_json::from_str(&cache_content).unwrap();
    let packages = cache["packages"].as_object().unwrap();
    assert_eq!(packages.len(), 1);
    assert_eq!(
        packages["@test/sample-workflow"]["version"],
        "0.1.0"
    );
}
