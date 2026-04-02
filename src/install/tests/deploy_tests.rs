use std::cell::Cell;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tempfile::tempdir;

use crate::artifact::Artifact;
use crate::backend::{claude::ClaudeBackend, Backend, DeployedArtifact};
use crate::config::Config;
use crate::error::{RenkeiError, Result};
use crate::install::{install_local, InstallOptions};
use crate::manifest::RequestedScope;

use super::helpers::{
    make_backend_test_pkg, make_multi_backend_pkg, AgentsFakeBackend, FailingBackend,
    ReadsAgentsSkillsBackend,
};

#[test]
fn test_rollback_cleans_partial_deploy() {
    let home = tempdir().unwrap();
    let pkg = tempdir().unwrap();

    fs::write(
        pkg.path().join("renkei.json"),
        r#"{"name":"@test/rollback","version":"1.0.0","description":"test","author":"t","license":"MIT","backends":["claude"]}"#,
    )
    .unwrap();

    let skills_dir = pkg.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::write(skills_dir.join("lint.md"), "# Lint").unwrap();
    fs::write(skills_dir.join("review.md"), "# Review").unwrap();

    let agents_dir = pkg.path().join("agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join("deploy.md"), "# Deploy").unwrap();

    let config = Config::with_home_dir(home.path().to_path_buf());
    let backend = FailingBackend {
        fail_on: 2,
        call_count: Cell::new(0),
    };

    let options = InstallOptions {
        force: true,
        ..InstallOptions::local("/tmp".to_string())
    };
    let result = install_local(
        pkg.path(),
        &config,
        &[&backend as &dyn Backend],
        RequestedScope::Global,
        &options,
    );
    assert!(result.is_err());

    assert!(!home.path().join(".claude/agents/deploy.md").exists());
    assert!(!home
        .path()
        .join(".claude/skills/renkei-lint/SKILL.md")
        .exists());
    assert!(!home.path().join(".claude/skills/renkei-lint").exists());
}

#[test]
fn test_backend_detected_succeeds() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let pkg = make_backend_test_pkg(r#"["claude"]"#);

    let config = Config::with_home_dir(home.path().to_path_buf());
    let options = InstallOptions::local("/tmp".to_string());
    let result = install_local(
        pkg.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &options,
    );
    assert!(result.is_ok());
}

#[test]
fn test_backend_not_detected_fails() {
    let home = tempdir().unwrap();
    let pkg = make_backend_test_pkg(r#"["claude"]"#);

    let config = Config::with_home_dir(home.path().to_path_buf());
    let options = InstallOptions::local("/tmp".to_string());
    let empty_backends: Vec<&dyn Backend> = vec![];
    let result = install_local(
        pkg.path(),
        &config,
        &empty_backends,
        RequestedScope::Global,
        &options,
    );
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("No compatible backend"));
    assert!(err.contains("--force"));
}

#[test]
fn test_backend_force_bypasses_check() {
    let home = tempdir().unwrap();
    let pkg = make_backend_test_pkg(r#"["claude"]"#);

    let config = Config::with_home_dir(home.path().to_path_buf());
    let options = InstallOptions {
        force: true,
        ..InstallOptions::local("/tmp".to_string())
    };
    let result = install_local(
        pkg.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &options,
    );
    assert!(result.is_ok());
}

#[test]
fn test_backend_multi_with_partial_match() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let pkg = make_backend_test_pkg(r#"["claude","cursor"]"#);

    let config = Config::with_home_dir(home.path().to_path_buf());
    let options = InstallOptions::local("/tmp".to_string());
    let result = install_local(
        pkg.path(),
        &config,
        &[&ClaudeBackend as &dyn Backend],
        RequestedScope::Global,
        &options,
    );
    assert!(
        result.is_ok(),
        "Should succeed with at least one matching backend"
    );
}

// --- Deduplication tests ---

#[test]
fn test_dedup_skips_skill_when_agents_and_reads_agents_backend() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());

    let pkg = make_multi_backend_pkg(r#"["agents","reads-agents"]"#);
    let opts = InstallOptions {
        force: true,
        ..InstallOptions::local("/tmp".to_string())
    };

    struct TrackingBackend {
        skill_deploy_count: Arc<AtomicUsize>,
    }

    impl Backend for TrackingBackend {
        fn name(&self) -> &str {
            "agents"
        }

        fn detect_installed(&self, _: &Config) -> bool {
            true
        }

        fn deploy_skill(
            &self,
            artifact: &Artifact,
            config: &Config,
        ) -> Result<DeployedArtifact> {
            self.skill_deploy_count.fetch_add(1, Ordering::SeqCst);
            ClaudeBackend.deploy_skill(artifact, config)
        }

        fn deploy_agent(&self, _: &Artifact, _: &Config) -> Result<DeployedArtifact> {
            Err(RenkeiError::DeploymentFailed("unsupported".into()))
        }

        fn deploy_hook(&self, _: &Artifact, _: &Config) -> Result<DeployedArtifact> {
            Err(RenkeiError::DeploymentFailed("unsupported".into()))
        }

        fn register_mcp(
            &self,
            _: &serde_json::Value,
            _: &Config,
        ) -> Result<Vec<crate::mcp::DeployedMcpEntry>> {
            Err(RenkeiError::DeploymentFailed("unsupported".into()))
        }
    }

    let agents_skill_count = Arc::new(AtomicUsize::new(0));
    let agents_backend = TrackingBackend {
        skill_deploy_count: agents_skill_count.clone(),
    };

    let result = install_local(
        pkg.path(),
        &config,
        &[
            &agents_backend as &dyn Backend,
            &ReadsAgentsSkillsBackend as &dyn Backend,
        ],
        RequestedScope::Global,
        &opts,
    );
    assert!(result.is_ok(), "{:?}", result);

    assert_eq!(
        agents_skill_count.load(Ordering::SeqCst),
        1,
        "Agents backend should deploy skill once"
    );
}

#[test]
fn test_no_dedup_when_agents_not_in_active_set() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());

    let pkg = make_multi_backend_pkg(r#"["reads-agents"]"#);
    let opts = InstallOptions {
        force: true,
        ..InstallOptions::local("/tmp".to_string())
    };

    let result = install_local(
        pkg.path(),
        &config,
        &[&ReadsAgentsSkillsBackend as &dyn Backend],
        RequestedScope::Global,
        &opts,
    );
    assert!(result.is_ok(), "{:?}", result);
    assert!(home
        .path()
        .join(".claude/skills/renkei-check/SKILL.md")
        .exists());
}

#[test]
fn test_no_dedup_for_agent_artifacts() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());

    let pkg = make_multi_backend_pkg(r#"["agents","reads-agents"]"#);
    let opts = InstallOptions {
        force: true,
        ..InstallOptions::local("/tmp".to_string())
    };

    let result = install_local(
        pkg.path(),
        &config,
        &[
            &AgentsFakeBackend as &dyn Backend,
            &ReadsAgentsSkillsBackend as &dyn Backend,
        ],
        RequestedScope::Global,
        &opts,
    );
    assert!(result.is_ok(), "{:?}", result);

    assert!(home.path().join(".claude/agents/helper.md").exists());
}
