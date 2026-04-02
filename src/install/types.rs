use crate::conflict::Conflict;
use crate::error::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum SourceKind {
    Local,
    Git,
}

impl SourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SourceKind::Local => "local",
            SourceKind::Git => "git",
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub force: bool,
    pub source_kind: SourceKind,
    pub source_url: String,
    pub resolved: Option<String>,
    pub tag: Option<String>,
}

impl InstallOptions {
    pub fn local(source_path: String) -> Self {
        Self {
            force: false,
            source_kind: SourceKind::Local,
            source_url: source_path,
            resolved: None,
            tag: None,
        }
    }

    pub fn git(url: String, resolved: String, tag: Option<String>) -> Self {
        Self {
            force: false,
            source_kind: SourceKind::Git,
            source_url: url,
            resolved: Some(resolved),
            tag,
        }
    }
}

pub type ConflictResolver = dyn Fn(&Conflict) -> Result<Option<String>>;

/// Source metadata for lockfile-based installs.
/// Contains only the fields needed to reconstruct a PackageEntry,
/// without the `force` control flag of InstallOptions.
#[derive(Debug, Clone)]
pub struct SourceInfo {
    pub source_kind: SourceKind,
    pub source_url: String,
    pub resolved: Option<String>,
    pub tag: Option<String>,
}
