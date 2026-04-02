use std::fs;

use tempfile::tempdir;

use crate::backend::{claude::ClaudeBackend, Backend};
use crate::config::Config;
use crate::install::pipeline::CorePipeline;
use crate::manifest::RequestedScope;
use crate::package_store::PackageStore;

use super::helpers::{force_resolver, make_backend_test_pkg, make_pkg_with_skill};

// --- CorePipeline::discover tests ---

#[test]
fn test_discover_loads_manifest_and_artifacts() {
    let pkg = make_pkg_with_skill("@test/pipe", "review");
    let backends: Vec<&dyn Backend> = vec![&ClaudeBackend];

    let pipeline = CorePipeline::discover(pkg.path(), &backends, false).unwrap();

    assert_eq!(pipeline.manifest.full_name, "@test/pipe");
    assert_eq!(pipeline.manifest.version.to_string(), "1.0.0");
    assert_eq!(pipeline.active_backends.len(), 1);
    assert_eq!(pipeline.active_backends[0].name(), "claude");
}

#[test]
fn test_discover_filters_backends_without_force() {
    let pkg = make_backend_test_pkg(r#"["cursor"]"#);
    let backends: Vec<&dyn Backend> = vec![&ClaudeBackend];

    let result = CorePipeline::discover(pkg.path(), &backends, false);

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("No compatible backend"));
}

#[test]
fn test_discover_force_keeps_all_backends() {
    let pkg = make_backend_test_pkg(r#"["cursor"]"#);
    let backends: Vec<&dyn Backend> = vec![&ClaudeBackend];

    let pipeline = CorePipeline::discover(pkg.path(), &backends, true).unwrap();

    assert_eq!(pipeline.active_backends.len(), 1);
    assert_eq!(pipeline.active_backends[0].name(), "claude");
}

#[test]
fn test_discover_errors_on_missing_manifest() {
    let dir = tempdir().unwrap();
    let backends: Vec<&dyn Backend> = vec![&ClaudeBackend];

    let result = CorePipeline::discover(dir.path(), &backends, false);
    assert!(result.is_err());
}

#[test]
fn test_discover_errors_on_no_artifacts() {
    let pkg = tempdir().unwrap();
    fs::write(
        pkg.path().join("renkei.json"),
        r#"{"name":"@test/empty","version":"1.0.0","description":"t","author":"t","license":"MIT","backends":["claude"]}"#,
    )
    .unwrap();
    let backends: Vec<&dyn Backend> = vec![&ClaudeBackend];

    let result = CorePipeline::discover(pkg.path(), &backends, false);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("No artifacts found"));
}

// --- CorePipeline::cleanup_and_resolve tests ---

#[test]
fn test_cleanup_and_resolve_produces_resolved_artifacts() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());

    let pkg = make_pkg_with_skill("@test/pipe", "review");
    let backends: Vec<&dyn Backend> = vec![&ClaudeBackend];

    let pipeline = CorePipeline::discover(pkg.path(), &backends, false).unwrap();
    let mut store = PackageStore::load(&config).unwrap();

    let resolved = pipeline
        .cleanup_and_resolve(&mut store, &force_resolver, &config)
        .unwrap();

    assert!(!resolved.resolved.effective.is_empty());
    assert_eq!(resolved.manifest.full_name, "@test/pipe");
}

// --- ResolvedPipeline::deploy tests ---

#[test]
fn test_deploy_creates_files() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());

    let pkg = make_pkg_with_skill("@test/pipe", "review");
    let backends: Vec<&dyn Backend> = vec![&ClaudeBackend];

    let pipeline = CorePipeline::discover(pkg.path(), &backends, false).unwrap();
    let mut store = PackageStore::load(&config).unwrap();
    let resolved = pipeline
        .cleanup_and_resolve(&mut store, &force_resolver, &config)
        .unwrap();

    let deployment = resolved.deploy(&config).unwrap();

    assert_eq!(deployment.all_deployed.len(), 1);
    assert!(deployment.deployed_map.contains_key("claude"));
    assert!(home
        .path()
        .join(".claude/skills/renkei-review/SKILL.md")
        .exists());
}

// --- Full pipeline roundtrip ---

#[test]
fn test_full_pipeline_discover_resolve_deploy() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());

    let pkg = make_pkg_with_skill("@test/full", "lint");
    let backends: Vec<&dyn Backend> = vec![&ClaudeBackend];

    // Discover
    let pipeline = CorePipeline::discover(pkg.path(), &backends, false).unwrap();
    assert_eq!(pipeline.manifest.full_name, "@test/full");

    // Resolve
    let mut store = PackageStore::load(&config).unwrap();
    let resolved = pipeline
        .cleanup_and_resolve(&mut store, &force_resolver, &config)
        .unwrap();

    // Deploy
    let deployment = resolved.deploy(&config).unwrap();
    assert!(!deployment.all_deployed.is_empty());

    // Verify
    let skill_path = home.path().join(".claude/skills/renkei-lint/SKILL.md");
    assert!(skill_path.exists());
}

#[test]
fn test_pipeline_cleanup_removes_previous_install() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());
    let backends: Vec<&dyn Backend> = vec![&ClaudeBackend];

    // First install
    let pkg = make_pkg_with_skill("@test/up", "review");
    let pipeline = CorePipeline::discover(pkg.path(), &backends, false).unwrap();
    let mut store = PackageStore::load(&config).unwrap();
    let resolved = pipeline
        .cleanup_and_resolve(&mut store, &force_resolver, &config)
        .unwrap();
    let deployment = resolved.deploy(&config).unwrap();

    store.record_install(
        "@test/up",
        crate::install_cache::PackageEntry {
            version: "1.0.0".to_string(),
            source: "local".to_string(),
            source_path: "/tmp".to_string(),
            integrity: "abc".to_string(),
            archive_path: "/tmp/a.tar.gz".to_string(),
            deployed: deployment.deployed_map,
            resolved: None,
            tag: None,
        },
        false,
    );
    store.save(&config).unwrap();

    let skill_path = home.path().join(".claude/skills/renkei-review/SKILL.md");
    assert!(skill_path.exists());

    // Re-install: cleanup should remove old files before deploying new
    let pkg2 = make_pkg_with_skill("@test/up", "review");
    let pipeline2 = CorePipeline::discover(pkg2.path(), &backends, false).unwrap();
    let mut store2 = PackageStore::load(&config).unwrap();
    let resolved2 = pipeline2
        .cleanup_and_resolve(&mut store2, &force_resolver, &config)
        .unwrap();
    let deployment2 = resolved2.deploy(&config).unwrap();

    assert!(!deployment2.all_deployed.is_empty());
    assert!(skill_path.exists());
}
