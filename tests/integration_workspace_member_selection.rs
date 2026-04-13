use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn setup_claude_home(home: &Path) {
    fs::create_dir_all(home.join(".claude")).unwrap();
}

/// Build a local workspace directory with the given members, each shipping
/// one skill named after the member. Returns the workspace TempDir.
fn setup_local_workspace(members: &[(&str, &str, &str)]) -> tempfile::TempDir {
    let ws = tempdir().unwrap();
    let names: Vec<String> = members.iter().map(|(d, _, _)| format!("\"{d}\"")).collect();
    fs::write(
        ws.path().join("renkei.json"),
        format!(r#"{{ "workspace": [{}] }}"#, names.join(", ")),
    )
    .unwrap();
    for (dir, full_name, skill) in members {
        let mdir = ws.path().join(dir);
        let sdir = mdir.join("skills").join(skill);
        fs::create_dir_all(&sdir).unwrap();
        fs::write(
            mdir.join("renkei.json"),
            format!(
                r#"{{"name":"{full_name}","version":"1.0.0","description":"t","author":"t","license":"MIT","backends":["claude"]}}"#
            ),
        )
        .unwrap();
        fs::write(
            sdir.join("SKILL.md"),
            format!("---\nname: {skill}\ndescription: test\n---\nContent of {skill}",),
        )
        .unwrap();
    }
    ws
}

/// Create a bare git repo containing a workspace package.
/// Returns `(bare, file_url)`. The bare directory is kept alive by the caller.
fn setup_bare_workspace_repo(members: &[(&str, &str, &str)]) -> (tempfile::TempDir, String) {
    let bare = tempdir().unwrap();
    std::process::Command::new("git")
        .args(["init", "--bare"])
        .arg(bare.path())
        .output()
        .unwrap();

    let work = tempdir().unwrap();
    std::process::Command::new("git")
        .args(["clone"])
        .arg(bare.path())
        .arg(work.path())
        .output()
        .unwrap();
    for (k, v) in [("user.email", "test@test.com"), ("user.name", "Test")] {
        std::process::Command::new("git")
            .args(["config", k, v])
            .current_dir(work.path())
            .output()
            .unwrap();
    }

    let names: Vec<String> = members.iter().map(|(d, _, _)| format!("\"{d}\"")).collect();
    fs::write(
        work.path().join("renkei.json"),
        format!(r#"{{ "workspace": [{}] }}"#, names.join(", ")),
    )
    .unwrap();
    for (dir, full_name, skill) in members {
        let mdir = work.path().join(dir);
        let sdir = mdir.join("skills").join(skill);
        fs::create_dir_all(&sdir).unwrap();
        fs::write(
            mdir.join("renkei.json"),
            format!(
                r#"{{"name":"{full_name}","version":"1.0.0","description":"t","author":"t","license":"MIT","backends":["claude"]}}"#
            ),
        )
        .unwrap();
        fs::write(
            sdir.join("SKILL.md"),
            format!("---\nname: {skill}\ndescription: test\n---\nContent of {skill}",),
        )
        .unwrap();
    }
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(work.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(work.path())
        .output()
        .unwrap();
    let branch_output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(work.path())
        .output()
        .unwrap();
    let branch = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();
    std::process::Command::new("git")
        .args(["push", "origin", &branch])
        .current_dir(work.path())
        .output()
        .unwrap();

    let url = format!("file://{}", bare.path().display());
    (bare, url)
}

fn rk(home: &Path) -> Command {
    let mut c = Command::cargo_bin("rk").unwrap();
    c.env("HOME", home);
    c
}

#[test]
fn local_install_with_member_deploys_only_named() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    let ws = setup_local_workspace(&[
        ("member-a", "@t/member-a", "review"),
        ("member-b", "@t/member-b", "lint"),
    ]);

    rk(home.path())
        .arg("install")
        .arg("-g")
        .arg(ws.path())
        .arg("-m")
        .arg("member-a")
        .assert()
        .success()
        .stdout(predicate::str::contains("@t/member-a"))
        .stdout(predicate::str::contains("@t/member-b").not());

    assert!(home
        .path()
        .join(".claude/skills/review/SKILL.md")
        .exists());
    assert!(!home
        .path()
        .join(".claude/skills/lint/SKILL.md")
        .exists());

    let lockfile = fs::read_to_string(home.path().join(".renkei/rk.lock")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&lockfile).unwrap();
    assert_eq!(
        parsed["packages"]["@t/member-a"]["member"], "member-a",
        "lockfile should record the member name"
    );
    assert!(parsed["packages"].get("@t/member-b").is_none());
}

#[test]
fn local_install_csv_and_repeated_member_flags_are_equivalent() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    let ws = setup_local_workspace(&[
        ("member-a", "@t/member-a", "review"),
        ("member-b", "@t/member-b", "lint"),
    ]);

    // CSV form
    rk(home.path())
        .arg("install")
        .arg("-g")
        .arg(ws.path())
        .arg("-m")
        .arg("member-a,member-b")
        .assert()
        .success();
    assert!(home
        .path()
        .join(".claude/skills/review/SKILL.md")
        .exists());
    assert!(home
        .path()
        .join(".claude/skills/lint/SKILL.md")
        .exists());

    // Wipe and try repeated form
    fs::remove_dir_all(home.path().join(".claude/skills")).unwrap();
    fs::remove_dir_all(home.path().join(".renkei")).unwrap();
    setup_claude_home(home.path());

    rk(home.path())
        .arg("install")
        .arg("-g")
        .arg(ws.path())
        .arg("-m")
        .arg("member-a")
        .arg("-m")
        .arg("member-b")
        .assert()
        .success();
    assert!(home
        .path()
        .join(".claude/skills/review/SKILL.md")
        .exists());
    assert!(home
        .path()
        .join(".claude/skills/lint/SKILL.md")
        .exists());
}

#[test]
fn install_with_unknown_member_fails_and_lists_available() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    let ws = setup_local_workspace(&[
        ("member-a", "@t/member-a", "review"),
        ("member-b", "@t/member-b", "lint"),
    ]);

    rk(home.path())
        .arg("install")
        .arg("-g")
        .arg(ws.path())
        .arg("-m")
        .arg("bogus")
        .assert()
        .failure()
        .stderr(predicate::str::contains("bogus"))
        .stderr(predicate::str::contains("member-a"))
        .stderr(predicate::str::contains("member-b"));

    assert!(!home
        .path()
        .join(".claude/skills/review/SKILL.md")
        .exists());
    assert!(!home
        .path()
        .join(".claude/skills/lint/SKILL.md")
        .exists());
}

#[test]
fn member_flag_on_non_workspace_fails() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    let pkg = tempdir().unwrap();
    fs::write(
        pkg.path().join("renkei.json"),
        r#"{"name":"@t/single","version":"1.0.0","description":"t","author":"t","license":"MIT","backends":["claude"]}"#,
    )
    .unwrap();
    let sdir = pkg.path().join("skills/foo");
    fs::create_dir_all(&sdir).unwrap();
    fs::write(
        sdir.join("SKILL.md"),
        "---\nname: foo\ndescription: t\n---\n",
    )
    .unwrap();

    rk(home.path())
        .arg("install")
        .arg("-g")
        .arg(pkg.path())
        .arg("-m")
        .arg("foo")
        .assert()
        .failure()
        .stderr(predicate::str::contains("workspace"));
}

#[test]
fn member_flag_with_no_arg_install_fails() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());

    rk(home.path())
        .arg("install")
        .arg("-g")
        .arg("-m")
        .arg("anything")
        .assert()
        .failure()
        .stderr(predicate::str::contains("-m"))
        .stderr(predicate::str::contains("lockfile"));
}

#[test]
fn git_install_with_member_deploys_only_named() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    let (_bare, url) = setup_bare_workspace_repo(&[
        ("member-a", "@t/member-a", "review"),
        ("member-b", "@t/member-b", "lint"),
    ]);

    rk(home.path())
        .arg("install")
        .arg("-g")
        .arg(&url)
        .arg("-m")
        .arg("member-a")
        .assert()
        .success();

    assert!(home
        .path()
        .join(".claude/skills/review/SKILL.md")
        .exists());
    assert!(!home
        .path()
        .join(".claude/skills/lint/SKILL.md")
        .exists());

    let lockfile = fs::read_to_string(home.path().join(".renkei/rk.lock")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&lockfile).unwrap();
    assert_eq!(parsed["packages"]["@t/member-a"]["member"], "member-a");
    assert_eq!(parsed["packages"]["@t/member-a"]["source"], url);
    assert!(parsed["packages"]["@t/member-a"]["resolved"].is_string());
}

#[test]
fn git_lockfile_reinstall_honors_member() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    let (_bare, url) = setup_bare_workspace_repo(&[
        ("member-a", "@t/member-a", "review"),
        ("member-b", "@t/member-b", "lint"),
    ]);

    // Install both members (separate -m to exercise multi-flag), generating
    // a lockfile that has `member` set on each entry.
    rk(home.path())
        .arg("install")
        .arg("-g")
        .arg(&url)
        .arg("-m")
        .arg("member-a")
        .arg("-m")
        .arg("member-b")
        .assert()
        .success();
    let lockfile_before = fs::read_to_string(home.path().join(".renkei/rk.lock")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&lockfile_before).unwrap();
    assert_eq!(parsed["packages"]["@t/member-a"]["member"], "member-a");
    assert_eq!(parsed["packages"]["@t/member-b"]["member"], "member-b");

    // Wipe deployed skills + cached archives, keep the lockfile.
    fs::remove_dir_all(home.path().join(".claude/skills")).unwrap();
    let cache_dir = home.path().join(".renkei");
    for entry in fs::read_dir(&cache_dir).unwrap() {
        let p = entry.unwrap().path();
        if p.is_dir() && p.file_name().unwrap() != "projects" {
            fs::remove_dir_all(&p).unwrap();
        }
        if p.is_file() && p.file_name().unwrap() == "install-cache.json" {
            fs::remove_file(&p).unwrap();
        }
    }

    // Replay from lockfile — must clone again and resolve <clone>/<member>.
    rk(home.path())
        .arg("install")
        .arg("-g")
        .assert()
        .success()
        .stdout(predicate::str::contains("Restoring"));

    assert!(home
        .path()
        .join(".claude/skills/review/SKILL.md")
        .exists());
    assert!(home
        .path()
        .join(".claude/skills/lint/SKILL.md")
        .exists());
}

#[test]
fn list_shows_member_suffix() {
    let home = tempdir().unwrap();
    setup_claude_home(home.path());
    let ws = setup_local_workspace(&[("member-a", "@t/member-a", "review")]);

    rk(home.path())
        .arg("install")
        .arg("-g")
        .arg(ws.path())
        .arg("-m")
        .arg("member-a")
        .assert()
        .success();

    rk(home.path())
        .arg("list")
        .arg("-g")
        .assert()
        .success()
        .stdout(predicate::str::contains("#member-a"));
}
