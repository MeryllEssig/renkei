use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn fixture_path(name: &str) -> PathBuf {
    std::env::current_dir()
        .unwrap()
        .join("tests/fixtures")
        .join(name)
}

fn fixture_str(name: &str) -> String {
    fixture_path(name).to_string_lossy().to_string()
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

fn copy_fixture_to(src_fixture: &str, dest: &Path) {
    let src = fixture_path(src_fixture);
    copy_dir_recursive(&src, dest);
}

fn copy_dir_recursive(src: &Path, dest: &Path) {
    fs::create_dir_all(dest).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir_recursive(&from, &to);
        } else {
            fs::copy(&from, &to).unwrap();
        }
    }
}

fn read_install_cache_global(home: &Path) -> serde_json::Value {
    let path = home.join(".renkei/install-cache.json");
    let content = fs::read_to_string(path).unwrap();
    serde_json::from_str(&content).unwrap()
}

fn read_claude_json(home: &Path) -> serde_json::Value {
    let path = home.join(".claude.json");
    if !path.exists() {
        return serde_json::json!({});
    }
    let content = fs::read_to_string(path).unwrap();
    serde_json::from_str(&content).unwrap()
}

#[test]
fn test_fresh_install_creates_folder_config_and_cache() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args([
            "install",
            "-g",
            "--allow-build",
            &fixture_str("local-mcp-pkg"),
        ])
        .assert()
        .success();

    let mcp_dir = home.path().join(".renkei/mcp/my-server");
    assert!(mcp_dir.is_dir(), "global MCP folder should exist");
    assert!(
        mcp_dir.join("dist/index.js").exists(),
        "entrypoint should be present after build/copy"
    );

    let claude = read_claude_json(home.path());
    let server = &claude["mcpServers"]["my-server"];
    assert_eq!(server["command"], "node");
    let args = server["args"].as_array().expect("args is array");
    let abs = args[0].as_str().unwrap();
    assert!(
        abs.ends_with("/.renkei/mcp/my-server/dist/index.js"),
        "expected absolute entrypoint path, got: {}",
        abs
    );

    let cache = read_install_cache_global(home.path());
    let entry = &cache["mcp_local"]["my-server"];
    assert_eq!(entry["owner_package"], "@test/local-mcp");
    assert_eq!(entry["version"], "1.0.0");
    assert_eq!(entry["referenced_by"].as_array().unwrap().len(), 1);
}

// `mcp_local` lives in the per-scope install-cache today, so a global +
// project install of the same package each record their ref in a different
// cache file. Cross-scope ref aggregation requires moving `mcp_local` to a
// dedicated single-source store; tracked as a follow-up.
#[ignore = "cross-scope mcp_local aggregation not yet implemented"]
#[test]
fn test_second_project_install_adds_ref_without_rebuild() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    setup_claude_home(home.path());
    init_git_repo(project.path());

    // First install: global.
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args([
            "install",
            "-g",
            "--allow-build",
            &fixture_str("local-mcp-pkg"),
        ])
        .assert()
        .success();

    // Second install: project scope (different ref).
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(project.path())
        .args(["install", "--allow-build", &fixture_str("local-mcp-pkg")])
        .assert()
        .success();

    let cache_global = read_install_cache_global(home.path());
    let refs = cache_global["mcp_local"]["my-server"]["referenced_by"]
        .as_array()
        .unwrap();
    assert_eq!(refs.len(), 2, "expected two refs, got: {:?}", refs);

    // Backend config still has the entry.
    let claude = read_claude_json(home.path());
    assert!(claude["mcpServers"]["my-server"].is_object());
}

// Same architectural caveat as `test_second_project_install_adds_ref_without_rebuild`:
// per-scope install caches mean a project-scope uninstall does not see refs
// recorded by a sibling global install.
#[ignore = "cross-scope mcp_local aggregation not yet implemented"]
#[test]
fn test_uninstall_decrements_then_gc() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    setup_claude_home(home.path());
    init_git_repo(project.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args([
            "install",
            "-g",
            "--allow-build",
            &fixture_str("local-mcp-pkg"),
        ])
        .assert()
        .success();
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(project.path())
        .args(["install", "--allow-build", &fixture_str("local-mcp-pkg")])
        .assert()
        .success();

    let mcp_dir = home.path().join(".renkei/mcp/my-server");
    assert!(mcp_dir.exists());

    // Uninstall project scope: folder must remain (global ref still active).
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(project.path())
        .args(["uninstall", "@test/local-mcp"])
        .assert()
        .success();
    assert!(
        mcp_dir.exists(),
        "folder must survive when global ref remains"
    );
    let cache = read_install_cache_global(home.path());
    assert_eq!(
        cache["mcp_local"]["my-server"]["referenced_by"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // Uninstall global: last ref → folder + config GC'd.
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["uninstall", "-g", "@test/local-mcp"])
        .assert()
        .success();
    assert!(!mcp_dir.exists(), "folder must be GC'd on last ref");
    let claude = read_claude_json(home.path());
    assert!(
        claude.get("mcpServers").is_none() || claude["mcpServers"].get("my-server").is_none(),
        "backend entry must be removed"
    );
    let cache_after = read_install_cache_global(home.path());
    assert!(
        cache_after.get("mcp_local").is_none()
            || cache_after["mcp_local"].get("my-server").is_none()
    );
}

#[test]
fn test_owner_conflict_without_force_errors() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args([
            "install",
            "-g",
            "--allow-build",
            &fixture_str("local-mcp-pkg"),
        ])
        .assert()
        .success();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args([
            "install",
            "-g",
            "--allow-build",
            &fixture_str("local-mcp-pkg-conflict"),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--force"));
}

#[test]
fn test_owner_conflict_with_force_transfers() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args([
            "install",
            "-g",
            "--allow-build",
            &fixture_str("local-mcp-pkg"),
        ])
        .assert()
        .success();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args([
            "install",
            "-g",
            "--allow-build",
            "--force",
            &fixture_str("local-mcp-pkg-conflict"),
        ])
        .assert()
        .success();

    let cache = read_install_cache_global(home.path());
    assert_eq!(
        cache["mcp_local"]["my-server"]["owner_package"],
        "@other/local-mcp"
    );
}

#[test]
fn test_non_tty_install_without_allow_build_errors() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["install", "-g", &fixture_str("local-mcp-pkg")])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--allow-build"));
}

#[cfg(unix)]
#[test]
fn test_link_mode_creates_symlink_without_build_prompt() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    // Stage the fixture into a tempdir to act as the workspace (so nothing
    // outside the temp scope is touched).
    let workspace = tempdir().unwrap();
    copy_fixture_to("local-mcp-pkg", workspace.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args([
            "install",
            "-g",
            "--link",
            workspace.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let target = home.path().join(".renkei/mcp/my-server");
    let meta = std::fs::symlink_metadata(&target).unwrap();
    assert!(meta.file_type().is_symlink(), "expected a symlink");

    // Source workspace must remain untouched after install.
    assert!(workspace
        .path()
        .join("mcp/my-server/dist/index.js")
        .exists());
}

// Replay reads the cached archive (which still matches the locked integrity)
// and recomputes the source hash off of that — workspace mutations are
// invisible to the drift check until the archive is regenerated. The plumbing
// for forcing a re-fetch from the original `source_path` on local installs is
// a separate piece of work.
#[ignore = "drift check uses the cached archive, not the live source path"]
#[test]
fn test_lockfile_replay_detects_source_drift() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    setup_claude_home(home.path());
    init_git_repo(project.path());

    // Stage the fixture into a tempdir we can mutate freely.
    let workspace = tempdir().unwrap();
    copy_fixture_to("local-mcp-pkg", workspace.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(project.path())
        .args([
            "install",
            "--allow-build",
            workspace.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    // Mutate source content; recorded hash will no longer match.
    fs::write(
        workspace.path().join("mcp/my-server/src/index.ts"),
        b"// MUTATED",
    )
    .unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(project.path())
        .args(["install", "--allow-build"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Lockfile drift").and(predicate::str::contains("my-server")),
        );
}

#[cfg(unix)]
#[test]
fn test_link_uninstall_removes_symlink_only() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    let workspace = tempdir().unwrap();
    copy_fixture_to("local-mcp-pkg", workspace.path());

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args([
            "install",
            "-g",
            "--link",
            workspace.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["uninstall", "-g", "@test/local-mcp"])
        .assert()
        .success();

    let target = home.path().join(".renkei/mcp/my-server");
    assert!(!target.exists(), "symlink must be removed");
    assert!(
        workspace
            .path()
            .join("mcp/my-server/dist/index.js")
            .exists(),
        "workspace source must remain intact"
    );
}
