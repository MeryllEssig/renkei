//! End-to-end tests for `messages.preinstall` and `messages.postinstall`.
//!
//! Cargo runs these binaries with stdin redirected, i.e. **non-TTY**. That
//! means a package declaring `messages.preinstall` will *fail* the install
//! unless `--yes` is passed — which is the production guarantee we want
//! (no silent prompt skips in CI). Tests asserting the success path always
//! pass `--yes`; tests asserting the non-TTY error omit it.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn setup_claude_home(home: &Path) {
    fs::create_dir_all(home.join(".claude")).unwrap();
}

/// Render the optional `, "messages": { ... }` JSON fragment that gets
/// spliced into a renkei.json string. Returns an empty string when both
/// inputs are `None`, so callers can interpolate unconditionally.
fn build_messages_json(preinstall: Option<&str>, postinstall: Option<&str>) -> String {
    let mut parts = Vec::new();
    if let Some(p) = preinstall {
        parts.push(format!(
            r#""preinstall": {}"#,
            serde_json::to_string(p).unwrap()
        ));
    }
    if let Some(p) = postinstall {
        parts.push(format!(
            r#""postinstall": {}"#,
            serde_json::to_string(p).unwrap()
        ));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(r#", "messages": {{ {} }}"#, parts.join(", "))
    }
}

/// Write a renkei.json + a single SKILL.md inside `pkg_dir`.
fn write_pkg(
    pkg_dir: &Path,
    full_name: &str,
    skill: &str,
    preinstall: Option<&str>,
    postinstall: Option<&str>,
) {
    let messages = build_messages_json(preinstall, postinstall);
    fs::write(
        pkg_dir.join("renkei.json"),
        format!(
            r#"{{"name":"{full_name}","version":"1.0.0","description":"t","author":"t","license":"MIT","backends":["claude"]{messages}}}"#
        ),
    )
    .unwrap();
    let sdir = pkg_dir.join("skills").join(skill);
    fs::create_dir_all(&sdir).unwrap();
    fs::write(
        sdir.join("SKILL.md"),
        format!("---\nname: {skill}\ndescription: test\n---\nContent of {skill}"),
    )
    .unwrap();
}

/// Build a single-skill package directory with optional pre/post messages.
fn build_pkg(
    full_name: &str,
    skill: &str,
    preinstall: Option<&str>,
    postinstall: Option<&str>,
) -> tempfile::TempDir {
    let pkg = tempdir().unwrap();
    write_pkg(pkg.path(), full_name, skill, preinstall, postinstall);
    pkg
}

/// Build a workspace where each member has its own pre/post pair.
fn build_workspace(
    members: &[(&str, &str, &str, Option<&str>, Option<&str>)],
) -> tempfile::TempDir {
    let ws = tempdir().unwrap();
    let dirs: Vec<String> = members
        .iter()
        .map(|(d, _, _, _, _)| format!("\"{d}\""))
        .collect();
    fs::write(
        ws.path().join("renkei.json"),
        format!(r#"{{"workspace":[{}]}}"#, dirs.join(", ")),
    )
    .unwrap();
    for (dir, full_name, skill, pre, post) in members {
        let mdir = ws.path().join(dir);
        fs::create_dir_all(&mdir).unwrap();
        write_pkg(&mdir, full_name, skill, *pre, *post);
    }
    ws
}

// --- Single-package preinstall ---

#[test]
fn single_local_preinstall_with_yes_installs() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    let pkg = build_pkg(
        "@test/pre-yes",
        "skill1",
        Some("Configure the X server before installing."),
        None,
    );

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["install", "-g", "--yes"])
        .arg(pkg.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("@test/pre-yes"));

    assert!(home
        .path()
        .join(".claude/skills/skill1/SKILL.md")
        .exists());
}

#[test]
fn single_local_preinstall_in_non_tty_without_yes_errors_with_hint() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    let pkg = build_pkg(
        "@test/pre-noyes",
        "skill2",
        Some("Read the README first."),
        None,
    );

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["install", "-g"])
        .arg(pkg.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes"))
        .stderr(predicate::str::contains("non-interactive"));

    // Nothing should have been deployed.
    assert!(!home
        .path()
        .join(".claude/skills/skill2/SKILL.md")
        .exists());
    // No lockfile entry either.
    assert!(!home.path().join(".renkei/rk.lock").exists());
}

#[test]
fn single_local_no_messages_does_not_prompt_in_non_tty() {
    // Sanity: a package with no messages should install fine in non-TTY mode
    // without --yes. Verifies the silent fast-path isn't broken by Phase 4.
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    let pkg = build_pkg("@test/no-msg", "skill3", None, None);

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["install", "-g"])
        .arg(pkg.path())
        .assert()
        .success();

    assert!(home
        .path()
        .join(".claude/skills/skill3/SKILL.md")
        .exists());
}

// --- Single-package postinstall ---

#[test]
fn single_local_postinstall_renders_after_done_line() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    let pkg = build_pkg(
        "@test/post1",
        "skill4",
        None,
        Some("Run `rk doctor` and restart Claude Code."),
    );

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["install", "-g"])
        .arg(pkg.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Done."))
        .stdout(predicate::str::contains("Postinstall notice:"))
        .stdout(predicate::str::contains("Run `rk doctor`"));
}

#[test]
fn single_local_postinstall_renders_after_required_env_warnings() {
    // Spec: render order is `Done.` → requiredEnv warnings → postinstall.
    // This guards against future refactors that interleave the two.
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    let pkg = tempdir().unwrap();
    fs::write(
        pkg.path().join("renkei.json"),
        r#"{"name":"@test/order","version":"1.0.0","description":"t","author":"t","license":"MIT","backends":["claude"],"requiredEnv":{"RK_TEST_NEVER_SET":"Sentinel env var that should always be missing"},"messages":{"postinstall":"After-env post-step sentinel."}}"#,
    )
    .unwrap();
    let sdir = pkg.path().join("skills/orderskill");
    fs::create_dir_all(&sdir).unwrap();
    fs::write(
        sdir.join("SKILL.md"),
        "---\nname: orderskill\ndescription: t\n---\nbody",
    )
    .unwrap();

    let assert = Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .env_remove("RK_TEST_NEVER_SET")
        .args(["install", "-g"])
        .arg(pkg.path())
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let env_pos = stdout
        .find("RK_TEST_NEVER_SET")
        .expect("env warning should appear in stdout");
    let post_pos = stdout
        .find("Postinstall notice:")
        .expect("postinstall block should appear in stdout");
    assert!(
        env_pos < post_pos,
        "env warning ({env_pos}) must precede postinstall block ({post_pos})\n---\n{stdout}\n---"
    );
}

// --- Workspace ---

#[test]
fn workspace_two_members_with_preinstall_show_single_block_and_one_yes_bypasses() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    let ws = build_workspace(&[
        ("a", "@test/ws-a", "ska", Some("Need MCP server X"), None),
        ("b", "@test/ws-b", "skb", Some("Need env var Y"), None),
    ]);

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["install", "-g", "--yes"])
        .arg(ws.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("@test/ws-a"))
        .stdout(predicate::str::contains("@test/ws-b"));

    assert!(home
        .path()
        .join(".claude/skills/ska/SKILL.md")
        .exists());
    assert!(home
        .path()
        .join(".claude/skills/skb/SKILL.md")
        .exists());
}

#[test]
fn workspace_preinstall_in_non_tty_without_yes_errors_and_installs_nothing() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    let ws = build_workspace(&[
        ("a", "@test/ws-na", "skna", Some("X required"), None),
        ("b", "@test/ws-nb", "sknb", None, None),
    ]);

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["install", "-g"])
        .arg(ws.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes"));

    // Neither member should have been deployed (refusal short-circuits the batch).
    assert!(!home
        .path()
        .join(".claude/skills/skna/SKILL.md")
        .exists());
    assert!(!home
        .path()
        .join(".claude/skills/sknb/SKILL.md")
        .exists());
}

#[test]
fn workspace_postinstall_blocks_render_at_end_with_member_labels() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    let ws = build_workspace(&[
        ("a", "@test/wp-a", "spa", None, Some("First post-step.")),
        ("b", "@test/wp-b", "spb", None, Some("Second post-step.")),
    ]);

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["install", "-g"])
        .arg(ws.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("@test/wp-a"))
        .stdout(predicate::str::contains("@test/wp-b"))
        .stdout(predicate::str::contains("First post-step."))
        .stdout(predicate::str::contains("Second post-step."));
}

#[test]
fn workspace_postinstall_only_renders_for_members_that_declare_one() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    let ws = build_workspace(&[
        ("a", "@test/po-a", "poa", None, Some("Only A speaks.")),
        ("b", "@test/po-b", "pob", None, None),
    ]);

    let assert = Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["install", "-g"])
        .arg(ws.path())
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    // Only one Postinstall notice block should appear.
    let count = stdout.matches("Postinstall notice:").count();
    assert_eq!(
        count, 1,
        "expected exactly one postinstall block; got\n{stdout}"
    );
    assert!(stdout.contains("Only A speaks."));
}

// --- Lockfile replay ---

#[test]
fn lockfile_replay_with_preinstall_requires_yes_in_non_tty() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    let pkg = build_pkg(
        "@test/lf-pre",
        "lfsk",
        Some("Preinstall message that should re-prompt on replay."),
        None,
    );

    // Initial install with --yes to seed the lockfile and archives.
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["install", "-g", "--yes"])
        .arg(pkg.path())
        .assert()
        .success();
    assert!(home.path().join(".renkei/rk.lock").exists());

    // Wipe deployed files; lockfile + archive remain.
    fs::remove_dir_all(home.path().join(".claude/skills")).unwrap();

    // Replay without --yes → should error in non-TTY.
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["install", "-g"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes"));

    // Replay with --yes → succeeds, re-deploys.
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["install", "-g", "--yes"])
        .assert()
        .success();
    assert!(home
        .path()
        .join(".claude/skills/lfsk/SKILL.md")
        .exists());
}

// --- Validation ---

#[test]
fn manifest_with_oversized_preinstall_fails_validation_pre_install() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    let big = "x".repeat(2001);
    let pkg = build_pkg("@test/big-msg", "bigsk", Some(&big), None);

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .args(["install", "-g", "--yes"])
        .arg(pkg.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("messages.preinstall"))
        .stderr(predicate::str::contains("2000"));

    // Nothing deployed.
    assert!(!home
        .path()
        .join(".claude/skills/bigsk/SKILL.md")
        .exists());
}
