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

fn write_global_cache(home: &Path, json: &str) {
    let cache_path = home.join(".renkei/install-cache.json");
    fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
    fs::write(&cache_path, json).unwrap();
}

// -- Empty / no packages --

#[test]
fn test_doctor_empty_global() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("doctor")
        .arg("-g")
        .assert()
        .success()
        .stdout(predicate::str::contains("No packages installed (global)."));
}

#[test]
fn test_doctor_empty_project() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    setup_claude_home(home.path());
    init_git_repo(project.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(project.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("No packages installed (project)."));
}

// -- Healthy after real install --

#[test]
fn test_doctor_healthy_after_install() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    // Install a real package
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("valid-package"))
        .assert()
        .success();

    // Doctor should be healthy
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("doctor")
        .arg("-g")
        .assert()
        .success()
        .stdout(predicate::str::contains("All healthy"))
        .stdout(predicate::str::contains("@test/sample-workflow"));
}

// -- Deleted deployed file --

#[test]
fn test_doctor_deleted_skill_file() {
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

    // Delete the deployed skill file
    let skill_path = home.path().join(".claude/skills/renkei-review/SKILL.md");
    assert!(skill_path.exists());
    fs::remove_file(&skill_path).unwrap();

    // Doctor should fail
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("doctor")
        .arg("-g")
        .assert()
        .failure()
        .stdout(predicate::str::contains("file missing"))
        .stdout(predicate::str::contains("FAIL"));
}

// -- Modified skill --

#[test]
fn test_doctor_modified_skill() {
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

    // Modify the deployed skill file
    let skill_path = home.path().join(".claude/skills/renkei-review/SKILL.md");
    fs::write(&skill_path, "# Modified content by user").unwrap();

    // Doctor should flag modification
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("doctor")
        .arg("-g")
        .assert()
        .failure()
        .stdout(predicate::str::contains("locally modified"))
        .stdout(predicate::str::contains("WARN"));
}

// -- Missing environment variable --

#[test]
fn test_doctor_missing_env_var() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    // Make sure the env var is NOT set
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .env_remove("RK_TEST_SECRET")
        .arg("install")
        .arg("-g")
        .arg(fixture_path("env-only"))
        .assert()
        .success();

    // Doctor should flag missing env var
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .env_remove("RK_TEST_SECRET")
        .arg("doctor")
        .arg("-g")
        .assert()
        .failure()
        .stdout(predicate::str::contains("RK_TEST_SECRET"));
}

// -- Missing hook in settings.json --

#[test]
fn test_doctor_missing_hook() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("hook-package"))
        .assert()
        .success();

    // Wipe hooks from settings.json
    let settings_path = home.path().join(".claude/settings.json");
    fs::write(&settings_path, r#"{"language":"French"}"#).unwrap();

    // Doctor should flag missing hooks
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("doctor")
        .arg("-g")
        .assert()
        .failure()
        .stdout(predicate::str::contains("missing from settings.json"));
}

// -- Missing MCP in claude.json --

#[test]
fn test_doctor_missing_mcp() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("mcp-package"))
        .assert()
        .success();

    // Wipe MCP from claude.json
    let config_path = home.path().join(".claude.json");
    fs::write(&config_path, r#"{"projects":{}}"#).unwrap();

    // Doctor should flag missing MCP
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("doctor")
        .arg("-g")
        .assert()
        .failure()
        .stdout(predicate::str::contains("test-server"))
        .stdout(predicate::str::contains("missing from claude.json"));
}

// -- Project scope --

#[test]
fn test_doctor_project_scope_healthy() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    setup_claude_home(home.path());
    init_git_repo(project.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(project.path())
        .arg("install")
        .arg(fixture_path("valid-package"))
        .assert()
        .success();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(project.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("All healthy"));
}

// -- Scope isolation --

#[test]
fn test_doctor_scope_isolation() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    setup_claude_home(home.path());
    init_git_repo(project.path());

    // Install globally
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("valid-package"))
        .assert()
        .success();

    // Project scope should be empty
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(project.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("No packages installed (project)."));
}

// -- Backend missing --

#[test]
fn test_doctor_backend_missing() {
    let home = tempdir().unwrap();
    // Do NOT create .claude dir

    let cache_json = r#"{
        "version": 2,
        "packages": {
            "@test/pkg": {
                "version": "1.0.0",
                "source": "local",
                "source_path": "/tmp",
                "integrity": "abc",
                "archive_path": "/nonexistent/archive.tar.gz",
                "deployed": {
                    "claude": {
                        "artifacts": [],
                        "mcp_servers": []
                    }
                }
            }
        }
    }"#;
    write_global_cache(home.path(), cache_json);

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("doctor")
        .arg("-g")
        .assert()
        .failure()
        .stdout(predicate::str::contains("FAIL"));
}
