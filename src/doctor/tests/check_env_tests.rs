use crate::cache;
use crate::config::Config;
use crate::doctor::checks;
use crate::doctor::types::DiagnosticKind;
use crate::manifest::{ManifestScope, ValidatedManifest};
use semver::Version;
use tempfile::tempdir;

use super::make_entry;

fn make_test_manifest() -> ValidatedManifest {
    ValidatedManifest {
        scope: "test".to_string(),
        short_name: "sample".to_string(),
        full_name: "@test/sample".to_string(),
        version: Version::new(0, 1, 0),
        install_scope: ManifestScope::Any,
        description: "test".to_string(),
        author: "tester".to_string(),
        license: "MIT".to_string(),
        backends: vec!["claude".to_string()],
    }
}

fn setup_package_with_skill(dir: &std::path::Path, skill_name: &str, content: &str) {
    std::fs::write(
        dir.join("renkei.json"),
        r#"{"name":"@test/sample","version":"0.1.0","description":"test","author":"tester","license":"MIT","backends":["claude"]}"#,
    ).unwrap();
    let skill_dir = dir.join("skills").join(skill_name);
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), content).unwrap();
}

fn setup_package_with_env(dir: &std::path::Path, required_env: &str) {
    std::fs::write(
        dir.join("renkei.json"),
        format!(
            r#"{{"name":"@test/sample","version":"0.1.0","description":"test","author":"tester","license":"MIT","backends":["claude"],"requiredEnv":{required_env}}}"#
        ),
    ).unwrap();
    let skill_dir = dir.join("skills/review");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "# Review").unwrap();
}

#[test]
fn test_env_vars_all_present() {
    let home = tempdir().unwrap();
    let pkg = tempdir().unwrap();

    unsafe { std::env::set_var("RK_DOCTOR_TEST_A", "val") };
    setup_package_with_env(pkg.path(), r#"{"RK_DOCTOR_TEST_A":"desc"}"#);
    let config = Config::with_home_dir(home.path().to_path_buf());
    let manifest = make_test_manifest();
    let (archive_path, _) = cache::create_archive(pkg.path(), &manifest, &config).unwrap();

    let mut entry = make_entry(vec![]);
    entry.archive_path = archive_path.to_string_lossy().to_string();

    let issues = checks::check_env_vars(&entry);
    assert!(issues.is_empty());
    unsafe { std::env::remove_var("RK_DOCTOR_TEST_A") };
}

#[test]
fn test_env_vars_missing() {
    let home = tempdir().unwrap();
    let pkg = tempdir().unwrap();

    std::env::remove_var("RK_DOCTOR_TEST_B");
    setup_package_with_env(pkg.path(), r#"{"RK_DOCTOR_TEST_B":"API key"}"#);
    let config = Config::with_home_dir(home.path().to_path_buf());
    let manifest = make_test_manifest();
    let (archive_path, _) = cache::create_archive(pkg.path(), &manifest, &config).unwrap();

    let mut entry = make_entry(vec![]);
    entry.archive_path = archive_path.to_string_lossy().to_string();

    let issues = checks::check_env_vars(&entry);
    assert_eq!(issues.len(), 1);
    assert!(
        matches!(&issues[0], DiagnosticKind::EnvVarMissing { var_name, description } if var_name == "RK_DOCTOR_TEST_B" && description == "API key")
    );
}

#[test]
fn test_env_vars_no_required_env() {
    let home = tempdir().unwrap();
    let pkg = tempdir().unwrap();

    setup_package_with_skill(pkg.path(), "review", "# Review");
    let config = Config::with_home_dir(home.path().to_path_buf());
    let manifest = make_test_manifest();
    let (archive_path, _) = cache::create_archive(pkg.path(), &manifest, &config).unwrap();

    let mut entry = make_entry(vec![]);
    entry.archive_path = archive_path.to_string_lossy().to_string();

    assert!(checks::check_env_vars(&entry).is_empty());
}

#[test]
fn test_env_vars_archive_missing() {
    let mut entry = make_entry(vec![]);
    entry.archive_path = "/nonexistent/archive.tar.gz".to_string();
    assert!(checks::check_env_vars(&entry).is_empty());
}
