use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

fn fixture_path(name: &str) -> String {
    let base = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures")
        .join(name);
    base.to_string_lossy().to_string()
}

#[test]
fn test_install_valid_local_package() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("valid-package"))
        .assert()
        .success()
        .stdout(predicate::str::contains("Done."));

    // Verify skill deployed
    let skill_path = home.path().join(".claude/skills/renkei-review/SKILL.md");
    assert!(
        skill_path.exists(),
        "SKILL.md should exist at {:?}",
        skill_path
    );

    // Verify content matches source
    let deployed = fs::read_to_string(&skill_path).unwrap();
    let source = fs::read_to_string(
        std::env::current_dir()
            .unwrap()
            .join("tests/fixtures/valid-package/skills/review.md"),
    )
    .unwrap();
    assert_eq!(deployed, source);

    // Verify archive exists
    let archive = home
        .path()
        .join(".renkei/archives/@test/sample-workflow/0.1.0.tar.gz");
    assert!(archive.exists(), "Archive should exist at {:?}", archive);

    // Verify install-cache.json
    let cache_path = home.path().join(".renkei/install-cache.json");
    assert!(cache_path.exists());
    let cache_content = fs::read_to_string(&cache_path).unwrap();
    let cache: serde_json::Value = serde_json::from_str(&cache_content).unwrap();
    assert!(cache["packages"]["@test/sample-workflow"].is_object());
    assert_eq!(
        cache["packages"]["@test/sample-workflow"]["version"],
        "0.1.0"
    );
    let artifacts = &cache["packages"]["@test/sample-workflow"]["deployed_artifacts"];
    assert_eq!(artifacts.as_array().unwrap().len(), 1);
    assert_eq!(artifacts[0]["name"], "review");
}

#[test]
fn test_install_multi_skill_package() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("multi-skill-package"))
        .assert()
        .success()
        .stdout(predicate::str::contains("2 artifact(s)"));

    assert!(home
        .path()
        .join(".claude/skills/renkei-review/SKILL.md")
        .exists());
    assert!(home
        .path()
        .join(".claude/skills/renkei-lint/SKILL.md")
        .exists());
}

#[test]
fn test_install_missing_name_fails() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("missing-name"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("name"));
}

#[test]
fn test_install_bad_scope_fails() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("bad-scope"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("@scope/name"));
}

#[test]
fn test_install_bad_semver_fails() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("bad-semver"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("not.a.version"));
}

#[test]
fn test_install_missing_fields_fails() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("missing-fields"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing field"));
}

#[test]
fn test_install_no_skills_fails() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("no-skills-package"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("No artifacts found"));
}

#[test]
fn test_install_nonexistent_path() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg("/nonexistent/path/to/nowhere")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Manifest not found"));
}

#[test]
fn test_install_mixed_package() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("mixed-package"))
        .assert()
        .success()
        .stdout(predicate::str::contains("3 artifact(s)"));

    // Verify skills deployed
    assert!(home
        .path()
        .join(".claude/skills/renkei-review/SKILL.md")
        .exists());
    assert!(home
        .path()
        .join(".claude/skills/renkei-lint/SKILL.md")
        .exists());

    // Verify agent deployed (no renkei- prefix, no subdirectory)
    assert!(home.path().join(".claude/agents/deploy.md").exists());

    // Verify install-cache.json
    let cache_path = home.path().join(".renkei/install-cache.json");
    let cache_content = fs::read_to_string(&cache_path).unwrap();
    let cache: serde_json::Value = serde_json::from_str(&cache_content).unwrap();
    let artifacts = &cache["packages"]["@test/mixed"]["deployed_artifacts"];
    assert_eq!(artifacts.as_array().unwrap().len(), 3);
}

#[test]
fn test_install_agent_deploy_path() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("mixed-package"))
        .assert()
        .success();

    let deployed = fs::read_to_string(home.path().join(".claude/agents/deploy.md")).unwrap();
    let source = fs::read_to_string(
        std::env::current_dir()
            .unwrap()
            .join("tests/fixtures/mixed-package/agents/deploy.md"),
    )
    .unwrap();
    assert_eq!(deployed, source);
}

#[test]
fn test_reinstall_replaces_artifacts() {
    let home = tempdir().unwrap();

    // First install
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("mixed-package"))
        .assert()
        .success();

    // Second install (reinstall)
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("mixed-package"))
        .assert()
        .success()
        .stdout(predicate::str::contains("3 artifact(s)"));

    // Verify still exactly 1 cache entry with 3 artifacts
    let cache_path = home.path().join(".renkei/install-cache.json");
    let cache_content = fs::read_to_string(&cache_path).unwrap();
    let cache: serde_json::Value = serde_json::from_str(&cache_content).unwrap();
    let packages = cache["packages"].as_object().unwrap();
    assert_eq!(packages.len(), 1);
    let artifacts = &packages["@test/mixed"]["deployed_artifacts"];
    assert_eq!(artifacts.as_array().unwrap().len(), 3);

    // Verify all files still exist after reinstall
    assert!(home
        .path()
        .join(".claude/skills/renkei-review/SKILL.md")
        .exists());
    assert!(home
        .path()
        .join(".claude/skills/renkei-lint/SKILL.md")
        .exists());
    assert!(home.path().join(".claude/agents/deploy.md").exists());
}

#[test]
fn test_reinstall_updates_cache() {
    let home = tempdir().unwrap();

    // Install valid-package first
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("valid-package"))
        .assert()
        .success();

    // Reinstall same package
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("valid-package"))
        .assert()
        .success();

    // Cache should have exactly 1 entry
    let cache_path = home.path().join(".renkei/install-cache.json");
    let cache_content = fs::read_to_string(&cache_path).unwrap();
    let cache: serde_json::Value = serde_json::from_str(&cache_content).unwrap();
    let packages = cache["packages"].as_object().unwrap();
    assert_eq!(packages.len(), 1);
    assert_eq!(packages["@test/sample-workflow"]["version"], "0.1.0");
}

// ---------------------------------------------------------------------------
// Phase 3: Hook integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_install_hook_package() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("hook-package"))
        .assert()
        .success()
        .stdout(predicate::str::contains("1 artifact(s)"));

    // Verify settings.json was created with correct structure
    let settings_path = home.path().join(".claude/settings.json");
    assert!(settings_path.exists());
    let settings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert!(settings["hooks"]["PreToolUse"].is_array());
    let pre_tool = &settings["hooks"]["PreToolUse"][0];
    assert_eq!(pre_tool["matcher"], "bash");
    assert_eq!(pre_tool["hooks"][0]["command"], "bash scripts/lint.sh");
    assert_eq!(pre_tool["hooks"][0]["timeout"], 5);
    assert_eq!(pre_tool["hooks"][0]["type"], "command");

    // Verify install-cache.json tracks the hook
    let cache_path = home.path().join(".renkei/install-cache.json");
    let cache: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&cache_path).unwrap()).unwrap();
    let artifacts = &cache["packages"]["@test/hook-pkg"]["deployed_artifacts"];
    assert_eq!(artifacts.as_array().unwrap().len(), 1);
    assert_eq!(artifacts[0]["artifact_type"], "hook");
    assert!(artifacts[0]["deployed_hooks"].is_array());
    assert_eq!(artifacts[0]["deployed_hooks"][0]["event"], "PreToolUse");
}

#[test]
fn test_install_hooks_preserve_existing_settings() {
    let home = tempdir().unwrap();

    // Pre-create settings.json with existing content
    let claude_dir = home.path().join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();
    fs::write(
        claude_dir.join("settings.json"),
        r#"{"permissions":{"allow":["Bash"]},"language":"French"}"#,
    )
    .unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("hook-package"))
        .assert()
        .success();

    let settings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(claude_dir.join("settings.json")).unwrap())
            .unwrap();
    // Hooks are added
    assert!(settings["hooks"]["PreToolUse"].is_array());
    // Existing settings preserved
    assert_eq!(settings["language"], "French");
    assert!(settings["permissions"]["allow"].is_array());
}

#[test]
fn test_install_hooks_append_to_existing() {
    let home = tempdir().unwrap();

    // Pre-create settings.json with an existing hook
    let claude_dir = home.path().join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();
    fs::write(
        claude_dir.join("settings.json"),
        r#"{"hooks":{"PreToolUse":[{"matcher":"Write","hooks":[{"type":"command","command":"existing.sh"}]}]}}"#,
    )
    .unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("hook-package"))
        .assert()
        .success();

    let settings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(claude_dir.join("settings.json")).unwrap())
            .unwrap();
    let pre_tool = settings["hooks"]["PreToolUse"].as_array().unwrap();
    // Should have 2 entries: existing + new
    assert_eq!(pre_tool.len(), 2);
    assert_eq!(pre_tool[0]["matcher"], "Write");
    assert_eq!(pre_tool[1]["matcher"], "bash");
}

#[test]
fn test_install_mixed_with_hooks() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("mixed-with-hooks"))
        .assert()
        .success()
        .stdout(predicate::str::contains("3 artifact(s)"));

    // Verify skill and agent deployed
    assert!(home
        .path()
        .join(".claude/skills/renkei-review/SKILL.md")
        .exists());
    assert!(home.path().join(".claude/agents/deploy.md").exists());

    // Verify hooks in settings.json (safety.json has 2 entries: before_tool + on_stop)
    let settings: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(home.path().join(".claude/settings.json")).unwrap(),
    )
    .unwrap();
    assert!(settings["hooks"]["PreToolUse"].is_array());
    assert!(settings["hooks"]["Stop"].is_array());

    // Verify install-cache has 3 artifacts (1 skill + 1 agent + 1 hook file)
    let cache: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(home.path().join(".renkei/install-cache.json")).unwrap(),
    )
    .unwrap();
    let artifacts = cache["packages"]["@test/mixed-hooks"]["deployed_artifacts"]
        .as_array()
        .unwrap();
    assert_eq!(artifacts.len(), 3);
}

#[test]
fn test_install_hooks_only_package() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("hooks-only"))
        .assert()
        .success()
        .stdout(predicate::str::contains("1 artifact(s)"));

    let settings: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(home.path().join(".claude/settings.json")).unwrap(),
    )
    .unwrap();
    assert!(settings["hooks"]["Notification"].is_array());
}

#[test]
fn test_install_bad_hook_event_fails() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("bad-hook-event"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown hook event"));
}

#[test]
fn test_reinstall_hook_package_no_duplication() {
    let home = tempdir().unwrap();

    // First install
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("hook-package"))
        .assert()
        .success();

    // Second install (reinstall)
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("hook-package"))
        .assert()
        .success();

    // Verify hooks are NOT duplicated in settings.json
    let settings: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(home.path().join(".claude/settings.json")).unwrap(),
    )
    .unwrap();
    let pre_tool = settings["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(
        pre_tool.len(),
        1,
        "Should have exactly 1 hook group after reinstall, got {}",
        pre_tool.len()
    );
}

// ---------------------------------------------------------------------------
// Phase 4: MCP + Environment variable integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_install_mcp_package() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("mcp-package"))
        .assert()
        .success()
        .stdout(predicate::str::contains("Done."));

    // Verify skill deployed
    assert!(home
        .path()
        .join(".claude/skills/renkei-api/SKILL.md")
        .exists());

    // Verify ~/.claude.json has MCP config
    let claude_json_path = home.path().join(".claude.json");
    assert!(claude_json_path.exists());
    let claude_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&claude_json_path).unwrap()).unwrap();
    assert_eq!(claude_json["mcpServers"]["test-server"]["command"], "node");
    assert_eq!(
        claude_json["mcpServers"]["test-server"]["args"][0],
        "server.js"
    );
    assert_eq!(
        claude_json["mcpServers"]["test-server"]["env"]["PORT"],
        "3000"
    );
}

#[test]
fn test_install_mcp_preserves_existing_servers() {
    let home = tempdir().unwrap();

    // Pre-populate ~/.claude.json with an existing server
    fs::write(
        home.path().join(".claude.json"),
        r#"{"mcpServers":{"existing-server":{"command":"keep","args":["me"]}}}"#,
    )
    .unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("mcp-package"))
        .assert()
        .success();

    let claude_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(home.path().join(".claude.json")).unwrap(),
    )
    .unwrap();
    // Both servers present
    assert_eq!(
        claude_json["mcpServers"]["existing-server"]["command"],
        "keep"
    );
    assert_eq!(claude_json["mcpServers"]["test-server"]["command"], "node");
}

#[test]
fn test_install_mcp_tracked_in_cache() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("mcp-package"))
        .assert()
        .success();

    let cache: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(home.path().join(".renkei/install-cache.json")).unwrap(),
    )
    .unwrap();
    let pkg = &cache["packages"]["@test/mcp-pkg"];
    let mcp_servers = pkg["deployed_mcp_servers"].as_array().unwrap();
    assert_eq!(mcp_servers.len(), 1);
    assert_eq!(mcp_servers[0], "test-server");
}

#[test]
fn test_reinstall_mcp_no_duplication() {
    let home = tempdir().unwrap();

    // First install
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("mcp-package"))
        .assert()
        .success();

    // Second install (reinstall)
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("mcp-package"))
        .assert()
        .success();

    // Verify only one MCP server entry
    let claude_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(home.path().join(".claude.json")).unwrap(),
    )
    .unwrap();
    let servers = claude_json["mcpServers"].as_object().unwrap();
    assert_eq!(servers.len(), 1);
    assert!(servers.contains_key("test-server"));
}

#[test]
fn test_install_env_warning_missing() {
    let home = tempdir().unwrap();

    // Ensure env vars are NOT set
    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .env_remove("RK_TEST_API_KEY")
        .env_remove("RK_TEST_DB_URL")
        .arg("install")
        .arg("-g")
        .arg(fixture_path("mcp-with-env"))
        .assert()
        .success()
        .stdout(predicate::str::contains("Missing environment variables"))
        .stdout(predicate::str::contains("RK_TEST_API_KEY"))
        .stdout(predicate::str::contains("RK_TEST_DB_URL"));
}

#[test]
fn test_install_env_no_warning_when_present() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .env("RK_TEST_API_KEY", "abc123")
        .env("RK_TEST_DB_URL", "postgres://localhost/test")
        .arg("install")
        .arg("-g")
        .arg(fixture_path("mcp-with-env"))
        .assert()
        .success()
        .stdout(predicate::str::contains("Missing environment variables").not());
}

#[test]
fn test_install_env_partial_warning() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .env("RK_TEST_API_KEY", "present")
        .env_remove("RK_TEST_DB_URL")
        .arg("install")
        .arg("-g")
        .arg(fixture_path("mcp-with-env"))
        .assert()
        .success()
        .stdout(predicate::str::contains("RK_TEST_DB_URL"))
        .stdout(predicate::str::contains("RK_TEST_API_KEY").not());
}

#[test]
fn test_install_without_mcp_no_claude_json() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .arg("install")
        .arg("-g")
        .arg(fixture_path("valid-package"))
        .assert()
        .success();

    // ~/.claude.json should NOT be created for packages without MCP
    assert!(!home.path().join(".claude.json").exists());
}

#[test]
fn test_install_env_only_no_mcp() {
    let home = tempdir().unwrap();

    Command::cargo_bin("rk")
        .unwrap()
        .env("HOME", home.path())
        .env_remove("RK_TEST_SECRET")
        .arg("install")
        .arg("-g")
        .arg(fixture_path("env-only"))
        .assert()
        .success()
        .stdout(predicate::str::contains("RK_TEST_SECRET"));

    // No ~/.claude.json since no MCP
    assert!(!home.path().join(".claude.json").exists());

    // But skill is deployed
    assert!(home
        .path()
        .join(".claude/skills/renkei-check/SKILL.md")
        .exists());
}
