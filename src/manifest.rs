use semver::Version;
use serde::Deserialize;
use std::path::Path;

use crate::error::{RenkeiError, Result};

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub license: String,
    pub backends: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub mcp: Option<serde_json::Value>,
    #[serde(rename = "requiredEnv", default)]
    pub required_env: Option<serde_json::Value>,
    #[serde(default)]
    pub workspace: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct ValidatedManifest {
    pub scope: String,
    pub short_name: String,
    pub full_name: String,
    pub version: Version,
    pub description: String,
    pub author: String,
    pub license: String,
    pub backends: Vec<String>,
}

impl Manifest {
    pub fn from_path(path: &Path) -> Result<Self> {
        let manifest_path = path.join("renkei.json");
        if !manifest_path.exists() {
            return Err(RenkeiError::ManifestNotFound(manifest_path));
        }
        let content = std::fs::read_to_string(&manifest_path)?;
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
            description: self.description.clone(),
            author: self.author.clone(),
            license: self.license.clone(),
            backends: self.backends.clone(),
        })
    }
}

fn parse_scoped_name(name: &str) -> Result<(String, String)> {
    if !name.starts_with('@') {
        return Err(RenkeiError::InvalidScope {
            name: name.to_string(),
        });
    }
    let without_at = &name[1..];
    let parts: Vec<&str> = without_at.splitn(2, '/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(RenkeiError::InvalidScope {
            name: name.to_string(),
        });
    }
    let scope = parts[0];
    let short_name = parts[1];

    let valid_chars =
        |s: &str| s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid_chars(scope) || !valid_chars(short_name) {
        return Err(RenkeiError::InvalidScope {
            name: name.to_string(),
        });
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
        let json =
            r#"{"name":"@t/n","version":"1.0.0","author":"a","license":"MIT","backends":["claude"]}"#;
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
}
