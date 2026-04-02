use std::fs;

use tempfile::tempdir;

use crate::backend::{claude::ClaudeBackend, Backend};
use crate::config::Config;
use crate::install::{install_local_with_resolver, InstallOptions};
use crate::lockfile::Lockfile;
use crate::manifest::RequestedScope;

use super::helpers::{force_resolver, make_pkg_with_skill};

#[test]
fn test_install_writes_lockfile_global() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());

    let pkg = make_pkg_with_skill("@test/lockpkg", "review");
    let opts = InstallOptions::local("/tmp/lockpkg".to_string());
    install_local_with_resolver(
        pkg.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts,
        &force_resolver,
    )
    .unwrap();

    let lockfile_path = home.path().join(".renkei/rk.lock");
    assert!(lockfile_path.exists(), "Lockfile should be created");

    let lockfile = Lockfile::load(&lockfile_path).unwrap();
    assert_eq!(lockfile.lockfile_version, 1);
    let entry = &lockfile.packages["@test/lockpkg"];
    assert_eq!(entry.version, "1.0.0");
    assert!(entry.integrity.starts_with("sha256-"));
}

#[test]
fn test_install_writes_lockfile_project() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let config = Config::for_project(home.path().to_path_buf(), project.path().to_path_buf());

    let pkg = make_pkg_with_skill("@test/lockpkg", "review");
    let opts = InstallOptions::local("/tmp/lockpkg".to_string());
    install_local_with_resolver(
        pkg.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Project,
        &opts,
        &force_resolver,
    )
    .unwrap();

    let lockfile_path = project.path().join("rk.lock");
    assert!(lockfile_path.exists(), "Project lockfile should be created");

    let lockfile = Lockfile::load(&lockfile_path).unwrap();
    assert!(lockfile.packages.contains_key("@test/lockpkg"));
}

#[test]
fn test_install_two_packages_lockfile_has_both() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());

    let pkg_a = make_pkg_with_skill("@test/pkg-a", "review");
    let pkg_b = make_pkg_with_skill("@test/pkg-b", "lint");
    let opts_a = InstallOptions::local("/tmp/a".to_string());
    let opts_b = InstallOptions::local("/tmp/b".to_string());

    install_local_with_resolver(
        pkg_a.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts_a,
        &force_resolver,
    )
    .unwrap();
    install_local_with_resolver(
        pkg_b.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts_b,
        &force_resolver,
    )
    .unwrap();

    let lockfile = Lockfile::load(&config.lockfile_path()).unwrap();
    assert_eq!(lockfile.packages.len(), 2);
    assert!(lockfile.packages.contains_key("@test/pkg-a"));
    assert!(lockfile.packages.contains_key("@test/pkg-b"));
}

#[test]
fn test_reinstall_updates_lockfile_entry() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());

    let pkg = make_pkg_with_skill("@test/pkg", "review");
    let opts = InstallOptions::local("/tmp".to_string());

    install_local_with_resolver(
        pkg.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts,
        &force_resolver,
    )
    .unwrap();
    install_local_with_resolver(
        pkg.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts,
        &force_resolver,
    )
    .unwrap();

    let lockfile = Lockfile::load(&config.lockfile_path()).unwrap();
    assert_eq!(lockfile.packages.len(), 1);
    assert_eq!(lockfile.packages["@test/pkg"].version, "1.0.0");
}
