use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn write_manifest(dir: &Path, version: &str) {
    fs::write(
        dir.join("renkei.json"),
        format!(
            r#"{{"name":"@test/sample","version":"{}","description":"test","author":"tester","license":"MIT","backends":["claude"]}}"#,
            version
        ),
    )
    .unwrap();
}

fn setup_full_package(dir: &Path) {
    write_manifest(dir, "1.0.0");
    let skill_dir = dir.join("skills/review");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), "# Review").unwrap();
    let agents = dir.join("agents");
    fs::create_dir_all(&agents).unwrap();
    fs::write(agents.join("deploy.md"), "# Deploy").unwrap();
    let hooks = dir.join("hooks");
    fs::create_dir_all(&hooks).unwrap();
    fs::write(hooks.join("lint.json"), "[]").unwrap();
    let scripts = dir.join("scripts");
    fs::create_dir_all(&scripts).unwrap();
    fs::write(scripts.join("build.sh"), "#!/bin/bash").unwrap();
}

fn tar_entry_names(archive_path: &Path) -> Vec<String> {
    let file = fs::File::open(archive_path).unwrap();
    let dec = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(dec);
    archive
        .entries()
        .unwrap()
        .map(|e| e.unwrap().path().unwrap().to_string_lossy().to_string())
        .collect()
}

#[test]
fn test_package_creates_archive() {
    let dir = tempdir().unwrap();
    setup_full_package(dir.path());

    Command::cargo_bin("rk")
        .unwrap()
        .current_dir(dir.path())
        .arg("package")
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"))
        .stdout(predicate::str::contains("test-sample-1.0.0.tar.gz"));

    let archive = dir.path().join("test-sample-1.0.0.tar.gz");
    assert!(archive.exists(), "Archive should exist at {:?}", archive);

    let entries = tar_entry_names(&archive);
    assert!(entries.contains(&"renkei.json".to_string()));
    assert!(entries.iter().any(|e| e.contains("review/SKILL.md")));
    assert!(entries.iter().any(|e| e.contains("deploy.md")));
    assert!(entries.iter().any(|e| e.contains("lint.json")));
    assert!(entries.iter().any(|e| e.contains("build.sh")));
}

#[test]
fn test_package_bump_patch() {
    let dir = tempdir().unwrap();
    setup_full_package(dir.path());

    Command::cargo_bin("rk")
        .unwrap()
        .current_dir(dir.path())
        .arg("package")
        .arg("--bump")
        .arg("patch")
        .assert()
        .success()
        .stdout(predicate::str::contains("test-sample-1.0.1.tar.gz"));

    assert!(dir.path().join("test-sample-1.0.1.tar.gz").exists());

    let content = fs::read_to_string(dir.path().join("renkei.json")).unwrap();
    let raw: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(raw["version"], "1.0.1");
}

#[test]
fn test_package_bump_minor() {
    let dir = tempdir().unwrap();
    setup_full_package(dir.path());

    Command::cargo_bin("rk")
        .unwrap()
        .current_dir(dir.path())
        .arg("package")
        .arg("--bump")
        .arg("minor")
        .assert()
        .success()
        .stdout(predicate::str::contains("test-sample-1.1.0.tar.gz"));

    assert!(dir.path().join("test-sample-1.1.0.tar.gz").exists());

    let content = fs::read_to_string(dir.path().join("renkei.json")).unwrap();
    let raw: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(raw["version"], "1.1.0");
}

#[test]
fn test_package_bump_major() {
    let dir = tempdir().unwrap();
    setup_full_package(dir.path());

    Command::cargo_bin("rk")
        .unwrap()
        .current_dir(dir.path())
        .arg("package")
        .arg("--bump")
        .arg("major")
        .assert()
        .success()
        .stdout(predicate::str::contains("test-sample-2.0.0.tar.gz"));

    assert!(dir.path().join("test-sample-2.0.0.tar.gz").exists());

    let content = fs::read_to_string(dir.path().join("renkei.json")).unwrap();
    let raw: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(raw["version"], "2.0.0");
}

#[test]
fn test_package_no_manifest() {
    let dir = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .current_dir(dir.path())
        .arg("package")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Manifest not found"));
}

#[test]
fn test_package_summary_output() {
    let dir = tempdir().unwrap();
    setup_full_package(dir.path());

    Command::cargo_bin("rk")
        .unwrap()
        .current_dir(dir.path())
        .arg("package")
        .assert()
        .success()
        .stdout(predicate::str::contains("renkei.json"))
        .stdout(predicate::str::contains("skills/review/SKILL.md"))
        .stdout(predicate::str::contains("agents/deploy.md"))
        .stdout(predicate::str::contains("hooks/lint.json"))
        .stdout(predicate::str::contains("scripts/build.sh"))
        .stdout(predicate::str::contains("files,"))
        .stdout(predicate::str::contains("SHA-256:"));
}

#[test]
fn test_package_excludes_non_standard() {
    let dir = tempdir().unwrap();
    setup_full_package(dir.path());
    fs::write(dir.path().join("README.md"), "# Readme").unwrap();
    fs::create_dir_all(dir.path().join("tests")).unwrap();
    fs::write(dir.path().join("tests/test.rs"), "fn main() {}").unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .current_dir(dir.path())
        .arg("package")
        .assert()
        .success();

    let archive = dir.path().join("test-sample-1.0.0.tar.gz");
    let entries = tar_entry_names(&archive);
    assert!(!entries.iter().any(|e| e.contains("README")));
    assert!(!entries.iter().any(|e| e.contains("test.rs")));
}

#[test]
fn test_package_includes_scripts() {
    let dir = tempdir().unwrap();
    write_manifest(dir.path(), "0.5.0");
    let scripts = dir.path().join("scripts");
    fs::create_dir_all(&scripts).unwrap();
    fs::write(scripts.join("setup.sh"), "#!/bin/bash\necho setup").unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .current_dir(dir.path())
        .arg("package")
        .assert()
        .success()
        .stdout(predicate::str::contains("scripts/setup.sh"));

    let archive = dir.path().join("test-sample-0.5.0.tar.gz");
    let entries = tar_entry_names(&archive);
    assert!(entries.iter().any(|e| e.contains("setup.sh")));
}
