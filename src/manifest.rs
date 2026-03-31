use semver::Version;
use serde::Deserialize;
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

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub license: String,
    pub backends: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub scope: ManifestScope,
    #[serde(default)]
    pub mcp: Option<serde_json::Value>,
    #[serde(rename = "requiredEnv", default)]
    pub required_env: Option<serde_json::Value>,
    #[allow(dead_code)]
    #[serde(default)]
    pub workspace: Option<Vec<String>>,
}

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

        if self.backends.is_empty() {
            return Err(RenkeiError::InvalidManifest(
                "backends must contain at least one entry".into(),
            ));
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
}

fn parse_scoped_name(name: &str) -> Result<(String, String)> {
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
    fn test_missing_backends_fails() {
        let json =
            r#"{"name":"@t/n","version":"1.0.0","description":"x","author":"a","license":"MIT"}"#;
        let result: std::result::Result<Manifest, _> = serde_json::from_str(json);
        assert!(result.is_err());
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
    fn test_empty_backends_fails() {
        let json = r#"{"name":"@t/n","version":"1.0.0","description":"x","author":"a","license":"MIT","backends":[]}"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        let err = m.validate().unwrap_err();
        assert!(err.to_string().contains("backends"));
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
}
