use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::error::{RenkeiError, Result};

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ManifestScope {
    #[default]
    Any,
    Global,
    Project,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RequestedScope {
    Project,
    Global,
}

pub fn validate_scope(manifest_scope: &ManifestScope, requested: RequestedScope) -> Result<()> {
    match (manifest_scope, requested) {
        (ManifestScope::Any, _) => Ok(()),
        (ManifestScope::Global, RequestedScope::Global) => Ok(()),
        (ManifestScope::Project, RequestedScope::Project) => Ok(()),
        (ManifestScope::Global, RequestedScope::Project) => Err(RenkeiError::ScopeConflict {
            message: "This package is global-only, use `rk install -g`".into(),
        }),
        (ManifestScope::Project, RequestedScope::Global) => Err(RenkeiError::ScopeConflict {
            message: "This package is project-only, remove `-g`".into(),
        }),
    }
}

/// Typed representation of a single `mcp.<name>` entry. Local-MCP fields
/// (`entrypoint`, `build`) are first-class; backend-native fields
/// (`command`, `args`, `env`, ...) are preserved verbatim via `extra`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpServer {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<Vec<Vec<String>>>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl McpServer {
    #[allow(dead_code)]
    pub fn is_local(&self) -> bool {
        self.entrypoint.is_some() || self.build.is_some()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Messages {
    #[serde(default)]
    pub preinstall: Option<String>,
    #[serde(default)]
    pub postinstall: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub license: String,
    #[serde(default)]
    pub backends: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub scope: ManifestScope,
    #[serde(default)]
    pub mcp: Option<HashMap<String, McpServer>>,
    #[serde(rename = "requiredEnv", default)]
    pub required_env: Option<serde_json::Value>,
    #[serde(default)]
    pub messages: Option<Messages>,
}

pub const MESSAGE_MAX_LEN: usize = 2000;

#[derive(Debug, Clone)]
pub struct ValidatedManifest {
    pub scope: String,
    pub short_name: String,
    pub full_name: String,
    pub version: Version,
    pub install_scope: ManifestScope,
    #[allow(dead_code)]
    pub description: String,
    #[allow(dead_code)]
    pub author: String,
    #[allow(dead_code)]
    pub license: String,
    #[allow(dead_code)]
    pub backends: Vec<String>,
}

impl Manifest {
    pub fn from_path(path: &Path) -> Result<Self> {
        let manifest_path = path.join("renkei.json");
        let content = std::fs::read_to_string(&manifest_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                RenkeiError::ManifestNotFound(manifest_path.clone())
            } else {
                RenkeiError::Io(e)
            }
        })?;
        let manifest: Manifest = serde_json::from_str(&content)?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<ValidatedManifest> {
        let (scope, short_name) = parse_scoped_name(&self.name)?;

        let version = Version::parse(&self.version).map_err(|e| RenkeiError::InvalidVersion {
            version: self.version.clone(),
            reason: e.to_string(),
        })?;

        if self.backends.iter().any(|b| b == "agents") {
            return Err(RenkeiError::InvalidManifest(
                "\"agents\" cannot be declared in backends — it is always active implicitly".into(),
            ));
        }

        if let Some(messages) = &self.messages {
            for (field, value) in [
                ("messages.preinstall", &messages.preinstall),
                ("messages.postinstall", &messages.postinstall),
            ] {
                if let Some(text) = value {
                    if text.chars().count() > MESSAGE_MAX_LEN {
                        return Err(RenkeiError::InvalidManifest(format!(
                            "{field} exceeds {MESSAGE_MAX_LEN} character limit"
                        )));
                    }
                }
            }
        }

        Ok(ValidatedManifest {
            scope,
            short_name,
            full_name: self.name.clone(),
            version,
            install_scope: self.scope.clone(),
            description: self.description.clone(),
            author: self.author.clone(),
            license: self.license.clone(),
            backends: self.backends.clone(),
        })
    }

    #[allow(dead_code)]
    pub fn is_skill_only(&self) -> bool {
        self.backends.is_empty()
    }

    /// Validate local-MCP conventions against the package on disk.
    ///
    /// Pure manifest checks live in `validate()`; this method requires a
    /// `package_root` because its rules cross-reference declared `mcp.<name>`
    /// entries with directories under `<package_root>/mcp/<name>/` and the
    /// presence of an `entrypoint` file when no `build` is declared.
    #[allow(dead_code)]
    pub fn validate_local_mcp(&self, package_root: &Path) -> Result<()> {
        let mcp_root = package_root.join("mcp");
        let declared: HashMap<String, &McpServer> = match &self.mcp {
            Some(m) => m.iter().map(|(k, v)| (k.clone(), v)).collect(),
            None if !mcp_root.exists() => return Ok(()),
            None => HashMap::new(),
        };

        for (name, server) in &declared {
            if let Some(build) = &server.build {
                if build.is_empty() {
                    return Err(RenkeiError::InvalidManifest(format!(
                        "mcp.{name}.build must be a non-empty array"
                    )));
                }
                for (idx, step) in build.iter().enumerate() {
                    if step.is_empty() {
                        return Err(RenkeiError::InvalidManifest(format!(
                            "mcp.{name}.build[{idx}] must be a non-empty argv array"
                        )));
                    }
                    for (tok_idx, tok) in step.iter().enumerate() {
                        if tok.is_empty() {
                            return Err(RenkeiError::InvalidManifest(format!(
                                "mcp.{name}.build[{idx}][{tok_idx}] must be a non-empty string"
                            )));
                        }
                    }
                }
            }

            if server.is_local() {
                let dir = mcp_root.join(name);
                if !dir.is_dir() {
                    return Err(RenkeiError::InvalidManifest(format!(
                        "mcp.{name} declares local MCP fields but `mcp/{name}/` directory is missing"
                    )));
                }
            }

            if server.build.is_some() && server.entrypoint.is_none() {
                return Err(RenkeiError::InvalidManifest(format!(
                    "mcp.{name} declares `build` but is missing required `entrypoint`"
                )));
            }

            if let (Some(ep), None) = (server.entrypoint.as_ref(), server.build.as_ref()) {
                let abs = mcp_root.join(name).join(ep);
                if !abs.is_file() {
                    return Err(RenkeiError::InvalidManifest(format!(
                        "mcp.{name}.entrypoint points to '{ep}' but file is missing and no `build` is declared"
                    )));
                }
            }
        }

        if mcp_root.is_dir() {
            for entry in std::fs::read_dir(&mcp_root)? {
                let entry = entry?;
                if !entry.file_type()?.is_dir() {
                    continue;
                }
                let dir_name = entry.file_name();
                let dir_name = dir_name.to_string_lossy().to_string();
                if !declared.contains_key(&dir_name) {
                    return Err(RenkeiError::InvalidManifest(format!(
                        "`mcp/{dir_name}/` exists on disk but mcp.{dir_name} is not declared in the manifest"
                    )));
                }
            }
        }

        Ok(())
    }
}

/// Detect collisions across workspace members: two members declaring the same
/// local MCP `<name>` are forbidden because all local MCPs deploy under a
/// single global path `~/.renkei/mcp/<name>/`.
#[allow(dead_code)]
pub fn validate_workspace_mcp_collisions(members: &[(String, &Manifest)]) -> Result<()> {
    let mut seen: HashMap<String, String> = HashMap::new();
    let mut collected: HashSet<&str> = HashSet::new();
    for (member_name, manifest) in members {
        if let Some(mcp) = &manifest.mcp {
            for name in mcp.keys() {
                if let Some(prev) = seen.get(name) {
                    if prev != member_name {
                        return Err(RenkeiError::InvalidManifest(format!(
                            "workspace members share MCP name '{name}' in '{prev}' and '{member_name}'"
                        )));
                    }
                }
                seen.insert(name.clone(), member_name.clone());
                collected.insert(name.as_str());
            }
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct WorkspaceManifest {
    workspace: Option<Vec<String>>,
}

/// Try to load a workspace member list from a directory's `renkei.json`.
/// Returns `Some(members)` if the manifest has a non-empty `workspace` field,
/// `None` otherwise.
pub fn try_load_workspace(path: &Path) -> Option<Vec<String>> {
    let manifest_path = path.join("renkei.json");
    let content = std::fs::read_to_string(manifest_path).ok()?;
    let ws: WorkspaceManifest = serde_json::from_str(&content).ok()?;
    ws.workspace.filter(|members| !members.is_empty())
}

pub fn parse_scoped_name(name: &str) -> Result<(String, String)> {
    let invalid = || RenkeiError::InvalidScope {
        name: name.to_string(),
    };

    let without_at = name.strip_prefix('@').ok_or_else(invalid)?;

    let (scope, short_name) = without_at
        .split_once('/')
        .filter(|(s, n)| !s.is_empty() && !n.is_empty())
        .ok_or_else(invalid)?;

    let valid_chars = |s: &str| {
        s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    };
    if !valid_chars(scope) || !valid_chars(short_name) {
        return Err(invalid());
    }

    Ok((scope.to_string(), short_name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_json() -> &'static str {
        r#"{
            "name": "@test/sample",
            "version": "1.0.0",
            "description": "A test package",
            "author": "tester",
            "license": "MIT",
            "backends": ["claude"]
        }"#
    }

    #[test]
    fn test_valid_manifest_parses() {
        let m: Manifest = serde_json::from_str(valid_json()).unwrap();
        assert_eq!(m.name, "@test/sample");
        assert_eq!(m.version, "1.0.0");
        assert_eq!(m.backends, vec!["claude"]);
    }

    #[test]
    fn test_valid_manifest_validates() {
        let m: Manifest = serde_json::from_str(valid_json()).unwrap();
        let v = m.validate().unwrap();
        assert_eq!(v.scope, "test");
        assert_eq!(v.short_name, "sample");
        assert_eq!(v.full_name, "@test/sample");
        assert_eq!(v.version, Version::new(1, 0, 0));
    }

    #[test]
    fn test_missing_name_fails() {
        let json = r#"{"version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude"]}"#;
        let result: std::result::Result<Manifest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_version_fails() {
        let json = r#"{"name":"@t/n","description":"x","author":"a","license":"MIT","backends":["claude"]}"#;
        let result: std::result::Result<Manifest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_description_fails() {
        let json = r#"{"name":"@t/n","version":"1.0.0","author":"a","license":"MIT","backends":["claude"]}"#;
        let result: std::result::Result<Manifest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_backends_parses_as_skill_only() {
        let json =
            r#"{"name":"@t/n","version":"1.0.0","description":"x","author":"a","license":"MIT"}"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        assert!(m.backends.is_empty());
        assert!(m.is_skill_only());
        m.validate().unwrap();
    }

    #[test]
    fn test_bad_scope_no_at() {
        let json = r#"{"name":"foo/bar","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude"]}"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        let err = m.validate().unwrap_err();
        assert!(err.to_string().contains("@scope/name"));
    }

    #[test]
    fn test_bad_scope_no_slash() {
        let json = r#"{"name":"@foobar","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude"]}"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        let err = m.validate().unwrap_err();
        assert!(err.to_string().contains("@scope/name"));
    }

    #[test]
    fn test_bad_scope_empty_parts() {
        let json = r#"{"name":"@/bar","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude"]}"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        assert!(m.validate().is_err());

        let json2 = r#"{"name":"@foo/","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude"]}"#;
        let m2: Manifest = serde_json::from_str(json2).unwrap();
        assert!(m2.validate().is_err());
    }

    #[test]
    fn test_bad_scope_invalid_chars() {
        let json = r#"{"name":"@foo bar/baz","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude"]}"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        assert!(m.validate().is_err());
    }

    #[test]
    fn test_bad_semver() {
        let json = r#"{"name":"@t/n","version":"not-a-version","description":"x","author":"a","license":"MIT","backends":["claude"]}"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        let err = m.validate().unwrap_err();
        assert!(err.to_string().contains("not-a-version"));
    }

    #[test]
    fn test_empty_backends_passes_as_skill_only() {
        let json = r#"{"name":"@t/n","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":[]}"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        assert!(m.is_skill_only());
        m.validate().unwrap();
    }

    #[test]
    fn test_manifest_with_claude_only_is_not_skill_only() {
        let m: Manifest = serde_json::from_str(valid_json()).unwrap();
        assert!(!m.is_skill_only());
        m.validate().unwrap();
    }

    #[test]
    fn test_manifest_with_agents_backend_rejected() {
        let json = r#"{"name":"@t/n","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["agents"]}"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        let err = m.validate().unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("\"agents\""),
            "error should quote agents: {msg}"
        );
        assert!(
            msg.contains("implicitly"),
            "error should explain implicit activation: {msg}"
        );
    }

    #[test]
    fn test_manifest_with_claude_and_agents_also_rejected() {
        let json = r#"{"name":"@t/n","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude","agents"]}"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        let err = m.validate().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("\"agents\""));
        assert!(msg.contains("implicitly"));
    }

    #[test]
    fn test_manifest_without_scope_defaults_to_any() {
        let m: Manifest = serde_json::from_str(valid_json()).unwrap();
        assert_eq!(m.scope, ManifestScope::Any);
    }

    #[test]
    fn test_manifest_scope_global_parses() {
        let json = r#"{"name":"@t/n","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude"],"scope":"global"}"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.scope, ManifestScope::Global);
    }

    #[test]
    fn test_manifest_scope_project_parses() {
        let json = r#"{"name":"@t/n","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude"],"scope":"project"}"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.scope, ManifestScope::Project);
    }

    #[test]
    fn test_manifest_scope_any_parses() {
        let json = r#"{"name":"@t/n","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude"],"scope":"any"}"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.scope, ManifestScope::Any);
    }

    #[test]
    fn test_manifest_scope_invalid_fails() {
        let json = r#"{"name":"@t/n","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude"],"scope":"local"}"#;
        let result: std::result::Result<Manifest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_scope_any_with_project() {
        assert!(validate_scope(&ManifestScope::Any, RequestedScope::Project).is_ok());
    }

    #[test]
    fn test_validate_scope_any_with_global() {
        assert!(validate_scope(&ManifestScope::Any, RequestedScope::Global).is_ok());
    }

    #[test]
    fn test_validate_scope_global_with_global() {
        assert!(validate_scope(&ManifestScope::Global, RequestedScope::Global).is_ok());
    }

    #[test]
    fn test_validate_scope_global_with_project_fails() {
        let err = validate_scope(&ManifestScope::Global, RequestedScope::Project).unwrap_err();
        assert!(err.to_string().contains("global-only"));
    }

    #[test]
    fn test_validate_scope_project_with_project() {
        assert!(validate_scope(&ManifestScope::Project, RequestedScope::Project).is_ok());
    }

    #[test]
    fn test_validate_scope_project_with_global_fails() {
        let err = validate_scope(&ManifestScope::Project, RequestedScope::Global).unwrap_err();
        assert!(err.to_string().contains("project-only"));
    }

    #[test]
    fn test_messages_absent_parses_as_none() {
        let m: Manifest = serde_json::from_str(valid_json()).unwrap();
        assert!(m.messages.is_none());
    }

    #[test]
    fn test_messages_with_both_fields_parses() {
        let json = r#"{"name":"@t/n","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude"],"messages":{"preinstall":"pre","postinstall":"post"}}"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        let msgs = m.messages.as_ref().unwrap();
        assert_eq!(msgs.preinstall.as_deref(), Some("pre"));
        assert_eq!(msgs.postinstall.as_deref(), Some("post"));
        m.validate().unwrap();
    }

    #[test]
    fn test_messages_with_only_preinstall_parses() {
        let json = r#"{"name":"@t/n","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude"],"messages":{"preinstall":"pre"}}"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        let msgs = m.messages.as_ref().unwrap();
        assert_eq!(msgs.preinstall.as_deref(), Some("pre"));
        assert!(msgs.postinstall.is_none());
    }

    #[test]
    fn test_messages_with_only_postinstall_parses() {
        let json = r#"{"name":"@t/n","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude"],"messages":{"postinstall":"post"}}"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        let msgs = m.messages.as_ref().unwrap();
        assert!(msgs.preinstall.is_none());
        assert_eq!(msgs.postinstall.as_deref(), Some("post"));
    }

    #[test]
    fn test_messages_preinstall_exceeding_limit_fails_validation() {
        let big = "x".repeat(MESSAGE_MAX_LEN + 1);
        let json = format!(
            r#"{{"name":"@t/n","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude"],"messages":{{"preinstall":{}}}}}"#,
            serde_json::to_string(&big).unwrap()
        );
        let m: Manifest = serde_json::from_str(&json).unwrap();
        let err = m.validate().unwrap_err();
        assert!(err.to_string().contains("messages.preinstall"));
        assert!(err.to_string().contains("2000"));
    }

    #[test]
    fn test_messages_postinstall_exceeding_limit_fails_validation() {
        let big = "y".repeat(MESSAGE_MAX_LEN + 1);
        let json = format!(
            r#"{{"name":"@t/n","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude"],"messages":{{"postinstall":{}}}}}"#,
            serde_json::to_string(&big).unwrap()
        );
        let m: Manifest = serde_json::from_str(&json).unwrap();
        let err = m.validate().unwrap_err();
        assert!(err.to_string().contains("messages.postinstall"));
    }

    #[test]
    fn test_messages_at_exact_limit_passes_validation() {
        let exact = "z".repeat(MESSAGE_MAX_LEN);
        let json = format!(
            r#"{{"name":"@t/n","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude"],"messages":{{"preinstall":{}}}}}"#,
            serde_json::to_string(&exact).unwrap()
        );
        let m: Manifest = serde_json::from_str(&json).unwrap();
        m.validate().unwrap();
    }

    #[test]
    fn test_try_load_workspace_returns_members() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("renkei.json"),
            r#"{ "workspace": ["member-a", "member-b"] }"#,
        )
        .unwrap();
        let members = try_load_workspace(dir.path()).unwrap();
        assert_eq!(members, vec!["member-a", "member-b"]);
    }

    #[test]
    fn test_try_load_workspace_returns_none_for_normal_manifest() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("renkei.json"), valid_json()).unwrap();
        assert!(try_load_workspace(dir.path()).is_none());
    }

    #[test]
    fn test_try_load_workspace_returns_none_for_empty_workspace() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("renkei.json"), r#"{ "workspace": [] }"#).unwrap();
        assert!(try_load_workspace(dir.path()).is_none());
    }

    #[test]
    fn test_try_load_workspace_returns_none_for_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(try_load_workspace(dir.path()).is_none());
    }

    fn manifest_with_mcp(mcp_json: &str) -> Manifest {
        let json = format!(
            r#"{{"name":"@t/n","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude"],"mcp":{mcp_json}}}"#
        );
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn test_external_only_mcp_parses_and_roundtrips() {
        let m = manifest_with_mcp(
            r#"{"weather":{"command":"node","args":["server.js"],"env":{"K":"v"}}}"#,
        );
        let server = m.mcp.as_ref().unwrap().get("weather").unwrap();
        assert!(!server.is_local());
        assert!(server.entrypoint.is_none());
        assert!(server.build.is_none());
        let v = serde_json::to_value(&m.mcp).unwrap();
        assert_eq!(v["weather"]["command"], "node");
        assert_eq!(v["weather"]["args"][0], "server.js");
        assert_eq!(v["weather"]["env"]["K"], "v");
    }

    #[test]
    fn test_local_mcp_with_build_and_entrypoint_parses() {
        let m = manifest_with_mcp(
            r#"{"srv":{"command":"node","entrypoint":"dist/index.js","build":[["bun","install"],["bun","run","build"]]}}"#,
        );
        let s = m.mcp.as_ref().unwrap().get("srv").unwrap();
        assert!(s.is_local());
        assert_eq!(s.entrypoint.as_deref(), Some("dist/index.js"));
        assert_eq!(s.build.as_ref().unwrap().len(), 2);
    }

    fn write_manifest_dir(dir: &Path, mcp: &str, sub_files: &[(&str, &str)]) {
        std::fs::write(
            dir.join("renkei.json"),
            format!(
                r#"{{"name":"@t/n","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude"],"mcp":{mcp}}}"#
            ),
        )
        .unwrap();
        for (rel, content) in sub_files {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }
    }

    #[test]
    fn test_validate_local_mcp_entrypoint_only_with_vendored_file_ok() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest_dir(
            dir.path(),
            r#"{"srv":{"command":"node","entrypoint":"index.js"}}"#,
            &[("mcp/srv/index.js", "console.log(1)")],
        );
        let m = Manifest::from_path(dir.path()).unwrap();
        m.validate_local_mcp(dir.path()).unwrap();
    }

    #[test]
    fn test_validate_local_mcp_entrypoint_only_missing_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest_dir(
            dir.path(),
            r#"{"srv":{"command":"node","entrypoint":"dist/index.js"}}"#,
            &[("mcp/srv/.keep", "")],
        );
        let m = Manifest::from_path(dir.path()).unwrap();
        let err = m.validate_local_mcp(dir.path()).unwrap_err();
        assert!(err.to_string().contains("entrypoint"));
        assert!(err.to_string().contains("dist/index.js"));
    }

    #[test]
    fn test_validate_local_mcp_build_without_entrypoint_errors() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest_dir(
            dir.path(),
            r#"{"srv":{"command":"node","build":[["bun","install"]]}}"#,
            &[("mcp/srv/.keep", "")],
        );
        let m = Manifest::from_path(dir.path()).unwrap();
        let err = m.validate_local_mcp(dir.path()).unwrap_err();
        assert!(err.to_string().contains("entrypoint"));
    }

    #[test]
    fn test_validate_local_mcp_dir_present_but_undeclared_errors() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest_dir(dir.path(), r#"{}"#, &[("mcp/foo/.keep", "")]);
        let m = Manifest::from_path(dir.path()).unwrap();
        let err = m.validate_local_mcp(dir.path()).unwrap_err();
        assert!(err.to_string().contains("mcp/foo"));
        assert!(err.to_string().contains("not declared"));
    }

    #[test]
    fn test_validate_local_mcp_declared_local_without_dir_errors() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest_dir(
            dir.path(),
            r#"{"srv":{"entrypoint":"x.js","build":[["bun","install"]]}}"#,
            &[],
        );
        let m = Manifest::from_path(dir.path()).unwrap();
        let err = m.validate_local_mcp(dir.path()).unwrap_err();
        assert!(err.to_string().contains("mcp/srv"));
    }

    #[test]
    fn test_validate_local_mcp_empty_build_array_errors() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest_dir(
            dir.path(),
            r#"{"srv":{"entrypoint":"x.js","build":[]}}"#,
            &[("mcp/srv/.keep", "")],
        );
        let m = Manifest::from_path(dir.path()).unwrap();
        let err = m.validate_local_mcp(dir.path()).unwrap_err();
        assert!(err.to_string().contains("non-empty"));
    }

    #[test]
    fn test_validate_local_mcp_empty_inner_argv_errors() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest_dir(
            dir.path(),
            r#"{"srv":{"entrypoint":"x.js","build":[[]]}}"#,
            &[("mcp/srv/.keep", "")],
        );
        let m = Manifest::from_path(dir.path()).unwrap();
        let err = m.validate_local_mcp(dir.path()).unwrap_err();
        assert!(err.to_string().contains("non-empty"));
    }

    #[test]
    fn test_validate_local_mcp_single_token_argv_ok() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest_dir(
            dir.path(),
            r#"{"srv":{"entrypoint":"x.js","build":[["bun"]]}}"#,
            &[("mcp/srv/.keep", "")],
        );
        let m = Manifest::from_path(dir.path()).unwrap();
        m.validate_local_mcp(dir.path()).unwrap();
    }

    #[test]
    fn test_validate_local_mcp_external_only_no_filesystem_check() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest_dir(
            dir.path(),
            r#"{"srv":{"command":"npx","args":["-y","@thing/mcp"]}}"#,
            &[],
        );
        let m = Manifest::from_path(dir.path()).unwrap();
        m.validate_local_mcp(dir.path()).unwrap();
    }

    fn make_member_manifest(mcp: &str) -> Manifest {
        let json = format!(
            r#"{{"name":"@t/n","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":["claude"],"mcp":{mcp}}}"#
        );
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn test_workspace_mcp_collision_detected() {
        let a = make_member_manifest(r#"{"shared":{"command":"node"}}"#);
        let b = make_member_manifest(r#"{"shared":{"command":"node"}}"#);
        let err =
            validate_workspace_mcp_collisions(&[("member-a".into(), &a), ("member-b".into(), &b)])
                .unwrap_err();
        assert!(err.to_string().contains("shared"));
        assert!(err.to_string().contains("member-a"));
        assert!(err.to_string().contains("member-b"));
    }

    #[test]
    fn test_workspace_mcp_no_collision_ok() {
        let a = make_member_manifest(r#"{"first":{"command":"node"}}"#);
        let b = make_member_manifest(r#"{"second":{"command":"node"}}"#);
        validate_workspace_mcp_collisions(&[("member-a".into(), &a), ("member-b".into(), &b)])
            .unwrap();
    }

    #[test]
    fn test_workspace_mcp_collision_ignores_member_with_no_mcp() {
        let a = make_member_manifest(r#"{"foo":{"command":"node"}}"#);
        let b: Manifest = serde_json::from_str(valid_json()).unwrap();
        validate_workspace_mcp_collisions(&[("member-a".into(), &a), ("member-b".into(), &b)])
            .unwrap();
    }
}
