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

fn write_project_cache(home: &Path, project_root: &Path, json: &str) {
    // Canonicalize to match git rev-parse --show-toplevel (resolves /var → /private/var on macOS)
    let canonical = project_root.canonicalize().unwrap();
    let slug = slug_path(&canonical);
    let cache_path = home.join(format!(".renkei/projects/{slug}/install-cache.json"));
    fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
    fs::write(&cache_path, json).unwrap();
}

fn slug_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    let without_leading = s.strip_prefix('/').unwrap_or(&s);
    without_leading.replace('/', "-")
}

const CACHE_TWO_PACKAGES: &str = r#"{
    "version": 1,
    "packages": {
        "@acme/review": {
            "version": "1.0.0",
            "source": "local",
            "source_path": "/tmp/review",
            "integrity": "abc",
            "archive_path": "/tmp/a.tar.gz",
            "deployed_artifacts": [
                {"artifact_type": "skill", "name": "review", "deployed_path": "/p/review"}
            ]
        },
        "@acme/deploy": {
            "version": "2.0.0",
            "source": "git",
            "source_path": "git@github.com:acme/deploy",
            "integrity": "def",
            "archive_path": "/tmp/b.tar.gz",
            "deployed_artifacts": [
                {"artifact_type": "agent", "name": "deploy", "deployed_path": "/p/deploy"}
            ],
            "resolved": "abcdef1234567890",
            "tag": "v2.0.0"
        }
    }
}"#;

#[test]
fn test_list_empty_global() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("list")
        .arg("-g")
        .assert()
        .success()
        .stdout(predicate::str::contains("No packages installed (global)."));
}

#[test]
fn test_list_empty_project() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    setup_claude_home(home.path());
    init_git_repo(project.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(project.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("No packages installed (project)."));
}

#[test]
fn test_list_global_with_packages() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    write_global_cache(home.path(), CACHE_TWO_PACKAGES);

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("list")
        .arg("-g")
        .assert()
        .success()
        .stdout(predicate::str::contains("@acme/review"))
        .stdout(predicate::str::contains("v1.0.0"))
        .stdout(predicate::str::contains("[local]"))
        .stdout(predicate::str::contains("@acme/deploy"))
        .stdout(predicate::str::contains("v2.0.0"))
        .stdout(predicate::str::contains("[git]"));
}

#[test]
fn test_list_project_with_packages() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    setup_claude_home(home.path());
    init_git_repo(project.path());

    let cache_json = r#"{
        "version": 1,
        "packages": {
            "@test/tools": {
                "version": "0.1.0",
                "source": "local",
                "source_path": "/tmp/tools",
                "integrity": "abc",
                "archive_path": "/tmp/a.tar.gz",
                "deployed_artifacts": [
                    {"artifact_type": "skill", "name": "lint", "deployed_path": "/p/lint"}
                ]
            }
        }
    }"#;
    write_project_cache(home.path(), project.path(), cache_json);

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(project.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("Installed packages (project):"))
        .stdout(predicate::str::contains("@test/tools"))
        .stdout(predicate::str::contains("v0.1.0"))
        .stdout(predicate::str::contains("1 skill"));
}

#[test]
fn test_list_scope_isolation() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    setup_claude_home(home.path());
    init_git_repo(project.path());

    // Global cache has @global/pkg
    let global_json = r#"{
        "version": 1,
        "packages": {
            "@global/pkg": {
                "version": "1.0.0",
                "source": "local",
                "source_path": "/tmp",
                "integrity": "abc",
                "archive_path": "/tmp/a.tar.gz",
                "deployed_artifacts": []
            }
        }
    }"#;
    write_global_cache(home.path(), global_json);

    // Project cache has @project/pkg
    let project_json = r#"{
        "version": 1,
        "packages": {
            "@project/pkg": {
                "version": "2.0.0",
                "source": "local",
                "source_path": "/tmp",
                "integrity": "def",
                "archive_path": "/tmp/b.tar.gz",
                "deployed_artifacts": []
            }
        }
    }"#;
    write_project_cache(home.path(), project.path(), project_json);

    // rk list -g shows only global
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("list")
        .arg("-g")
        .assert()
        .success()
        .stdout(predicate::str::contains("@global/pkg"))
        .stdout(predicate::str::contains("@project/pkg").not());

    // rk list shows only project
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(project.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("@project/pkg"))
        .stdout(predicate::str::contains("@global/pkg").not());
}

#[test]
fn test_list_project_outside_git_repo() {
    let home = tempdir().unwrap();
    let no_git = tempdir().unwrap();
    setup_claude_home(home.path());
    // no git init — not a repo

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(no_git.path())
        .env("GIT_CEILING_DIRECTORIES", no_git.path())
        .arg("list")
        .assert()
        .failure();
}

#[test]
fn test_list_mixed_sources() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    write_global_cache(home.path(), CACHE_TWO_PACKAGES);

    let output = Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("list")
        .arg("-g")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[local]"));
    assert!(stdout.contains("[git]"));
}

#[test]
fn test_list_git_shows_sha() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    write_global_cache(home.path(), CACHE_TWO_PACKAGES);

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("list")
        .arg("-g")
        .assert()
        .success()
        .stdout(predicate::str::contains("abcdef1"))
        .stdout(predicate::str::contains("v2.0.0 @ abcdef1"));
}

#[test]
fn test_list_after_real_install() {
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

    // Then list
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("list")
        .arg("-g")
        .assert()
        .success()
        .stdout(predicate::str::contains("@test/sample-workflow"))
        .stdout(predicate::str::contains("0.1.0"))
        .stdout(predicate::str::contains("1 skill"));
}
