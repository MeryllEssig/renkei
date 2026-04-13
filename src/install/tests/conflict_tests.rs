use std::fs;

use tempfile::tempdir;

use crate::backend::{claude::ClaudeBackend, Backend};
use crate::config::Config;
use crate::install::{default_resolver, install_local_with_resolver, InstallOptions};
use crate::install_cache::InstallCache;
use crate::manifest::RequestedScope;

use super::helpers::{error_resolver, force_resolver, make_pkg_with_skill, rename_resolver};

#[test]
fn test_conflict_force_overwrites() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());

    let pkg_a = make_pkg_with_skill("@test/conflict-a", "review");
    let opts_a = InstallOptions::local("/tmp/a".to_string());
    install_local_with_resolver(
        pkg_a.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts_a,
        &force_resolver,
    )
    .unwrap();

    let pkg_b = make_pkg_with_skill("@test/conflict-b", "review");
    let opts_b = InstallOptions::local("/tmp/b".to_string());
    install_local_with_resolver(
        pkg_b.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts_b,
        &force_resolver,
    )
    .unwrap();

    let skill_path = home.path().join(".claude/skills/review/SKILL.md");
    assert!(skill_path.exists());
    let content = fs::read_to_string(&skill_path).unwrap();
    assert!(content.contains("Content of review"));

    let cache = InstallCache::load(&config).unwrap();
    let a_entry = &cache.packages["@test/conflict-a"];
    assert_eq!(
        a_entry.all_artifacts().count(),
        0,
        "Package A should have no deployed artifacts after force overwrite"
    );

    let b_entry = &cache.packages["@test/conflict-b"];
    let b_arts: Vec<_> = b_entry.all_artifacts().collect();
    assert_eq!(b_arts.len(), 1);
    assert_eq!(b_arts[0].name, "review");
}

#[test]
fn test_conflict_error_resolver_aborts() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());

    let pkg_a = make_pkg_with_skill("@test/conflict-a", "review");
    let opts = InstallOptions::local("/tmp/a".to_string());
    install_local_with_resolver(
        pkg_a.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts,
        &force_resolver,
    )
    .unwrap();

    let pkg_b = make_pkg_with_skill("@test/conflict-b", "review");
    let opts_b = InstallOptions::local("/tmp/b".to_string());
    let result = install_local_with_resolver(
        pkg_b.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts_b,
        &error_resolver,
    );

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("review"));
    assert!(err.contains("@test/conflict-a"));
}

#[test]
fn test_conflict_rename_deploys_under_new_name() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());

    let pkg_a = make_pkg_with_skill("@test/conflict-a", "review");
    let opts = InstallOptions::local("/tmp/a".to_string());
    install_local_with_resolver(
        pkg_a.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts,
        &force_resolver,
    )
    .unwrap();

    let pkg_b = make_pkg_with_skill("@test/conflict-b", "review");
    let opts_b = InstallOptions::local("/tmp/b".to_string());
    let resolver = rename_resolver("review-v2");
    install_local_with_resolver(
        pkg_b.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts_b,
        &resolver,
    )
    .unwrap();

    let a_path = home.path().join(".claude/skills/review/SKILL.md");
    assert!(a_path.exists());

    let b_path = home.path().join(".claude/skills/review-v2/SKILL.md");
    assert!(b_path.exists());

    let content = fs::read_to_string(&b_path).unwrap();
    assert!(content.contains("name: review-v2"));
    assert!(content.contains("Content of review"));
}

#[test]
fn test_conflict_rename_tracks_original_name_in_cache() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());

    let pkg_a = make_pkg_with_skill("@test/conflict-a", "review");
    let opts = InstallOptions::local("/tmp/a".to_string());
    install_local_with_resolver(
        pkg_a.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts,
        &force_resolver,
    )
    .unwrap();

    let pkg_b = make_pkg_with_skill("@test/conflict-b", "review");
    let opts_b = InstallOptions::local("/tmp/b".to_string());
    let resolver = rename_resolver("review-v2");
    install_local_with_resolver(
        pkg_b.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts_b,
        &resolver,
    )
    .unwrap();

    let cache = InstallCache::load(&config).unwrap();
    let b_entry = &cache.packages["@test/conflict-b"];
    let b_arts: Vec<_> = b_entry.all_artifacts().collect();
    assert_eq!(b_arts[0].name, "review-v2");
    assert_eq!(b_arts[0].original_name.as_deref(), Some("review"));
}

#[test]
fn test_no_conflict_on_reinstall() {
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
        &error_resolver,
    )
    .unwrap();
    install_local_with_resolver(
        pkg.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts,
        &error_resolver,
    )
    .unwrap();
}

#[test]
fn test_no_conflict_different_skill_names() {
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
        &error_resolver,
    )
    .unwrap();
    install_local_with_resolver(
        pkg_b.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts_b,
        &error_resolver,
    )
    .unwrap();

    assert!(home.path().join(".claude/skills/review/SKILL.md").exists());
    assert!(home.path().join(".claude/skills/lint/SKILL.md").exists());
}

#[test]
fn test_default_resolver_auto_renames_with_scope() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());

    // Package "@acme/pkg-a" installs skill "review" first.
    let pkg_a = make_pkg_with_skill("@acme/pkg-a", "review");
    let opts_a = InstallOptions::local("/tmp/a".to_string());
    install_local_with_resolver(
        pkg_a.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts_a,
        &force_resolver,
    )
    .unwrap();

    // Second package from a DIFFERENT scope also ships "review".
    // With the default (non-force) resolver, it must auto-rename to
    // "{incoming_scope}-{name}" — here "widgetco-review".
    let pkg_b = make_pkg_with_skill("@widgetco/pkg-b", "review");
    let opts_b = InstallOptions::local("/tmp/b".to_string());
    let resolver = default_resolver(false, "widgetco");
    install_local_with_resolver(
        pkg_b.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts_b,
        &*resolver,
    )
    .unwrap();

    assert!(home.path().join(".claude/skills/review/SKILL.md").exists());
    let renamed = home.path().join(".claude/skills/widgetco-review/SKILL.md");
    assert!(renamed.exists());
    let content = fs::read_to_string(&renamed).unwrap();
    assert!(content.contains("name: widgetco-review"));

    let cache = InstallCache::load(&config).unwrap();
    let b_entry = &cache.packages["@widgetco/pkg-b"];
    let b_arts: Vec<_> = b_entry.all_artifacts().collect();
    assert_eq!(b_arts[0].name, "widgetco-review");
    assert_eq!(b_arts[0].original_name.as_deref(), Some("review"));
}

#[test]
fn test_residual_conflict_on_renamed_target_errors() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());

    // A package already ships an artifact literally named "widgetco-review"
    // (this is the target name the scope-rename would pick for the next install).
    let squatter = make_pkg_with_skill("@other/squatter", "widgetco-review");
    let opts = InstallOptions::local("/tmp/s".to_string());
    install_local_with_resolver(
        squatter.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts,
        &force_resolver,
    )
    .unwrap();

    // Another package holds the "review" name, so the new install will
    // collide on "review" and the default resolver will propose "widgetco-review"
    // — which is already taken by @other/squatter. Expect an explicit error.
    let holder = make_pkg_with_skill("@holder/pkg", "review");
    let opts_h = InstallOptions::local("/tmp/h".to_string());
    install_local_with_resolver(
        holder.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts_h,
        &force_resolver,
    )
    .unwrap();

    let pkg_b = make_pkg_with_skill("@widgetco/pkg-b", "review");
    let opts_b = InstallOptions::local("/tmp/b".to_string());
    let resolver = default_resolver(false, "widgetco");
    let result = install_local_with_resolver(
        pkg_b.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &opts_b,
        &*resolver,
    );

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("widgetco-review"));
    assert!(err.contains("@other/squatter"));
}
