use std::cell::Cell;
use std::collections::HashMap;
use std::fs;

use tempfile::tempdir;

use crate::artifact::{Artifact, ArtifactKind};
use crate::backend::{claude::ClaudeBackend, Backend, DeployedArtifact};
use crate::config::{BackendId, Config};
use crate::conflict;
use crate::error::{RenkeiError, Result};
use crate::install_cache::{BackendDeployment, DeployedArtifactEntry, InstallCache, PackageEntry};

// --- Fixture builders ---

pub fn make_cache_with_artifacts(artifacts: Vec<(ArtifactKind, &str, &str)>) -> InstallCache {
    let deployed_entries: Vec<DeployedArtifactEntry> = artifacts
        .into_iter()
        .map(|(kind, name, path)| DeployedArtifactEntry {
            artifact_type: kind,
            name: name.to_string(),
            deployed_path: path.to_string(),
            deployed_hooks: vec![],
            original_name: None,
        })
        .collect();
    let mut deployed = HashMap::new();
    deployed.insert(
        "claude".to_string(),
        BackendDeployment {
            artifacts: deployed_entries,
            mcp_servers: vec![],
        },
    );
    let mut packages = HashMap::new();
    packages.insert(
        "@test/pkg".to_string(),
        PackageEntry {
            version: "1.0.0".to_string(),
            source: "local".to_string(),
            source_path: "/tmp/pkg".to_string(),
            integrity: "abc".to_string(),
            archive_path: "/tmp/a.tar.gz".to_string(),
            deployed,
            resolved: None,
            tag: None,
            member: None,
        },
    );
    InstallCache {
        version: 2,
        packages,
    }
}

pub fn make_backend_test_pkg(backends_json: &str) -> tempfile::TempDir {
    let pkg = tempdir().unwrap();
    fs::write(
        pkg.path().join("renkei.json"),
        format!(
            r#"{{"name":"@test/pkg","version":"1.0.0","description":"t","author":"t","license":"MIT","backends":{backends_json}}}"#
        ),
    )
    .unwrap();
    let skill_dir = pkg.path().join("skills/check");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), "# Check").unwrap();
    pkg
}

pub fn make_pkg_with_skill(name: &str, skill_name: &str) -> tempfile::TempDir {
    let pkg = tempdir().unwrap();
    fs::write(
        pkg.path().join("renkei.json"),
        format!(
            r#"{{"name":"{name}","version":"1.0.0","description":"t","author":"t","license":"MIT","backends":["claude"]}}"#
        ),
    )
    .unwrap();
    let skill_dir = pkg.path().join("skills").join(skill_name);
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: {skill_name}\ndescription: test\n---\nContent of {skill_name}"),
    )
    .unwrap();
    pkg
}

pub fn make_multi_backend_pkg(backends_json: &str) -> tempfile::TempDir {
    let pkg = tempdir().unwrap();
    fs::write(
        pkg.path().join("renkei.json"),
        format!(
            r#"{{"name":"@test/pkg","version":"1.0.0","description":"t","author":"t","license":"MIT","backends":{backends_json}}}"#
        ),
    )
    .unwrap();
    let skill_dir = pkg.path().join("skills/check");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), "# Check").unwrap();
    let agents = pkg.path().join("agents");
    fs::create_dir_all(&agents).unwrap();
    fs::write(agents.join("helper.md"), "# Helper").unwrap();
    pkg
}

// --- Conflict resolvers ---

pub fn force_resolver(_: &conflict::Conflict) -> Result<Option<String>> {
    Ok(None)
}

pub fn error_resolver(c: &conflict::Conflict) -> Result<Option<String>> {
    Err(RenkeiError::ArtifactConflict {
        kind: c.artifact_kind.clone(),
        name: c.artifact_name.clone(),
        owner: c.owner_package.clone(),
    })
}

pub fn rename_resolver(
    new_name: &str,
) -> impl Fn(&conflict::Conflict) -> Result<Option<String>> + '_ {
    move |_| Ok(Some(new_name.to_string()))
}

// --- Mock backends ---

pub struct FailingBackend {
    pub fail_on: usize,
    pub call_count: Cell<usize>,
}

impl FailingBackend {
    fn try_call<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        let count = self.call_count.get();
        self.call_count.set(count + 1);
        if count >= self.fail_on {
            return Err(RenkeiError::DeploymentFailed("simulated failure".into()));
        }
        f()
    }
}

impl Backend for FailingBackend {
    fn name(&self) -> &str {
        "failing"
    }

    fn backend_id(&self) -> BackendId {
        BackendId::Claude
    }

    fn detect_installed(&self, _config: &Config) -> bool {
        true
    }

    fn deploy_skill(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        self.try_call(|| ClaudeBackend.deploy_skill(artifact, config))
    }

    fn deploy_agent(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        self.try_call(|| ClaudeBackend.deploy_agent(artifact, config))
    }

    fn deploy_hook(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        self.try_call(|| ClaudeBackend.deploy_hook(artifact, config))
    }

    fn register_mcp(
        &self,
        mcp_config: &serde_json::Value,
        config: &Config,
    ) -> Result<Vec<crate::mcp::DeployedMcpEntry>> {
        self.try_call(|| ClaudeBackend.register_mcp(mcp_config, config))
    }
}

pub struct ReadsAgentsSkillsBackend;

impl Backend for ReadsAgentsSkillsBackend {
    fn name(&self) -> &str {
        "reads-agents"
    }

    fn backend_id(&self) -> BackendId {
        BackendId::Claude
    }

    fn detect_installed(&self, _config: &Config) -> bool {
        true
    }

    fn reads_agents_skills(&self) -> bool {
        true
    }

    fn deploy_skill(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        ClaudeBackend.deploy_skill(artifact, config)
    }

    fn deploy_agent(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        ClaudeBackend.deploy_agent(artifact, config)
    }

    fn deploy_hook(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        ClaudeBackend.deploy_hook(artifact, config)
    }

    fn register_mcp(
        &self,
        mcp_config: &serde_json::Value,
        config: &Config,
    ) -> Result<Vec<crate::mcp::DeployedMcpEntry>> {
        ClaudeBackend.register_mcp(mcp_config, config)
    }
}

pub struct AgentsFakeBackend;

impl Backend for AgentsFakeBackend {
    fn name(&self) -> &str {
        "agents"
    }

    fn backend_id(&self) -> BackendId {
        BackendId::Agents
    }

    fn detect_installed(&self, _: &Config) -> bool {
        true
    }

    fn deploy_skill(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        ClaudeBackend.deploy_skill(artifact, config)
    }

    fn deploy_agent(&self, _artifact: &Artifact, _config: &Config) -> Result<DeployedArtifact> {
        Err(RenkeiError::DeploymentFailed("agents: unsupported".into()))
    }

    fn deploy_hook(&self, _artifact: &Artifact, _config: &Config) -> Result<DeployedArtifact> {
        Err(RenkeiError::DeploymentFailed("agents: unsupported".into()))
    }

    fn register_mcp(
        &self,
        _mcp_config: &serde_json::Value,
        _config: &Config,
    ) -> Result<Vec<crate::mcp::DeployedMcpEntry>> {
        Err(RenkeiError::DeploymentFailed("agents: unsupported".into()))
    }
}
