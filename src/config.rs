use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::error::{RenkeiError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendId {
    Claude,
    Cursor,
    Codex,
    Gemini,
    Agents,
}

#[derive(Debug, Clone)]
pub struct BackendDirs {
    pub root_dir: PathBuf,
    pub skills_dir: Option<PathBuf>,
    pub agents_dir: Option<PathBuf>,
    pub settings_path: Option<PathBuf>,
    pub config_path: Option<PathBuf>,
    pub hooks_path: Option<PathBuf>,
    pub mcp_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub home_dir: PathBuf,
    pub project_root: Option<PathBuf>,
}

impl Config {
    pub fn default_home_dir() -> PathBuf {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"))
    }

    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            home_dir: Self::default_home_dir(),
            project_root: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_home_dir(home_dir: PathBuf) -> Self {
        Self {
            home_dir,
            project_root: None,
        }
    }

    #[allow(dead_code)]
    pub fn for_project(home_dir: PathBuf, project_root: PathBuf) -> Self {
        Self {
            home_dir,
            project_root: Some(project_root),
        }
    }

    pub fn renkei_dir(&self) -> PathBuf {
        self.home_dir.join(".renkei")
    }

    /// Root directory under which local MCP server sources are deployed.
    /// Always under `~/.renkei/mcp/`, regardless of the active install scope —
    /// MCP registration in backends is global by nature (e.g. `~/.claude.json`).
    pub fn global_mcp_dir(&self) -> PathBuf {
        self.renkei_dir().join("mcp")
    }

    pub fn archives_dir(&self) -> PathBuf {
        self.renkei_dir().join("archives")
    }

    pub fn install_cache_path(&self) -> PathBuf {
        match self.project_root {
            Some(ref root) => self
                .renkei_dir()
                .join("projects")
                .join(Self::slug(root))
                .join("install-cache.json"),
            None => self.renkei_dir().join("install-cache.json"),
        }
    }

    pub fn slug(path: &Path) -> String {
        let s = path.to_string_lossy();
        let without_leading = s.strip_prefix('/').unwrap_or(&s);
        without_leading.replace('/', "-")
    }

    pub fn is_project(&self) -> bool {
        self.project_root.is_some()
    }

    pub fn scope_label(&self) -> &'static str {
        if self.is_project() {
            "project"
        } else {
            "global"
        }
    }

    pub fn lockfile_path(&self) -> PathBuf {
        match self.project_root {
            Some(ref root) => root.join("rk.lock"),
            None => self.renkei_dir().join("rk.lock"),
        }
    }

    /// Resolve a scoped path: project-local when in project scope, home-based otherwise.
    fn scoped(&self, dot_dir: &str, name: &str) -> PathBuf {
        match self.project_root {
            Some(ref root) => root.join(dot_dir).join(name),
            None => self.home_dir.join(dot_dir).join(name),
        }
    }

    /// Single entry point for all backend path resolution.
    pub fn backend(&self, id: BackendId) -> BackendDirs {
        match id {
            BackendId::Claude => BackendDirs {
                root_dir: self.home_dir.join(".claude"),
                skills_dir: Some(self.scoped(".claude", "skills")),
                agents_dir: Some(self.scoped(".claude", "agents")),
                settings_path: Some(self.home_dir.join(".claude").join("settings.json")),
                config_path: Some(self.home_dir.join(".claude.json")),
                hooks_path: None,
                mcp_path: None,
            },
            BackendId::Cursor => BackendDirs {
                root_dir: self.home_dir.join(".cursor"),
                skills_dir: Some(self.scoped(".cursor", "rules")),
                agents_dir: Some(self.scoped(".cursor", "agents")),
                settings_path: None,
                config_path: None,
                hooks_path: Some(self.scoped(".cursor", "hooks.json")),
                mcp_path: Some(self.scoped(".cursor", "mcp.json")),
            },
            BackendId::Codex => BackendDirs {
                root_dir: self.home_dir.join(".codex"),
                skills_dir: None,
                agents_dir: Some(self.scoped(".codex", "agents")),
                settings_path: None,
                config_path: Some(self.scoped(".codex", "config.toml")),
                hooks_path: Some(self.scoped(".codex", "hooks.json")),
                mcp_path: None,
            },
            BackendId::Gemini => BackendDirs {
                root_dir: self.home_dir.join(".gemini"),
                skills_dir: Some(self.scoped(".gemini", "skills")),
                agents_dir: Some(self.scoped(".gemini", "agents")),
                settings_path: Some(self.scoped(".gemini", "settings.json")),
                config_path: None,
                hooks_path: None,
                mcp_path: None,
            },
            BackendId::Agents => {
                let root = match self.project_root {
                    Some(ref root) => root.join(".agents"),
                    None => self.home_dir.join(".agents"),
                };
                BackendDirs {
                    skills_dir: Some(root.join("skills")),
                    root_dir: root,
                    agents_dir: None,
                    settings_path: None,
                    config_path: None,
                    hooks_path: None,
                    mcp_path: None,
                }
            }
        }
    }
}

pub fn detect_project_root() -> Result<PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .stderr(Stdio::null())
        .output()
        .map_err(|_| RenkeiError::NoProjectRoot)?;

    if !output.status.success() {
        return Err(RenkeiError::NoProjectRoot);
    }

    let path_str = String::from_utf8_lossy(&output.stdout);
    Ok(PathBuf::from(path_str.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slug_basic() {
        assert_eq!(
            Config::slug(Path::new("/Users/meryll/Projects/foo")),
            "Users-meryll-Projects-foo"
        );
    }

    #[test]
    fn test_slug_no_leading_slash() {
        assert_eq!(Config::slug(Path::new("tmp")), "tmp");
    }

    #[test]
    fn test_lockfile_path_global() {
        let config = Config::with_home_dir(PathBuf::from("/home/user"));
        assert_eq!(
            config.lockfile_path(),
            PathBuf::from("/home/user/.renkei/rk.lock")
        );
    }

    #[test]
    fn test_lockfile_path_project() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        assert_eq!(
            config.lockfile_path(),
            PathBuf::from("/projects/foo/rk.lock")
        );
    }

    #[test]
    fn test_install_cache_path_global() {
        let config = Config::with_home_dir(PathBuf::from("/home/user"));
        assert_eq!(
            config.install_cache_path(),
            PathBuf::from("/home/user/.renkei/install-cache.json")
        );
    }

    #[test]
    fn test_install_cache_path_project() {
        let config = Config::for_project(
            PathBuf::from("/home/user"),
            PathBuf::from("/Users/meryll/Projects/foo"),
        );
        assert_eq!(
            config.install_cache_path(),
            PathBuf::from(
                "/home/user/.renkei/projects/Users-meryll-Projects-foo/install-cache.json"
            )
        );
    }

    // --- BackendDirs boundary tests ---

    #[test]
    fn test_backend_claude_global() {
        let config = Config::with_home_dir(PathBuf::from("/home/user"));
        let dirs = config.backend(BackendId::Claude);
        assert_eq!(dirs.root_dir, PathBuf::from("/home/user/.claude"));
        assert_eq!(
            dirs.skills_dir.unwrap(),
            PathBuf::from("/home/user/.claude/skills")
        );
        assert_eq!(
            dirs.agents_dir.unwrap(),
            PathBuf::from("/home/user/.claude/agents")
        );
        assert_eq!(
            dirs.settings_path.unwrap(),
            PathBuf::from("/home/user/.claude/settings.json")
        );
        assert_eq!(
            dirs.config_path.unwrap(),
            PathBuf::from("/home/user/.claude.json")
        );
        assert!(dirs.hooks_path.is_none());
        assert!(dirs.mcp_path.is_none());
    }

    #[test]
    fn test_backend_claude_project() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        let dirs = config.backend(BackendId::Claude);
        assert_eq!(dirs.root_dir, PathBuf::from("/home/user/.claude"));
        assert_eq!(
            dirs.skills_dir.unwrap(),
            PathBuf::from("/projects/foo/.claude/skills")
        );
        assert_eq!(
            dirs.agents_dir.unwrap(),
            PathBuf::from("/projects/foo/.claude/agents")
        );
        assert_eq!(
            dirs.settings_path.unwrap(),
            PathBuf::from("/home/user/.claude/settings.json")
        );
        assert_eq!(
            dirs.config_path.unwrap(),
            PathBuf::from("/home/user/.claude.json")
        );
    }

    #[test]
    fn test_backend_cursor_global() {
        let config = Config::with_home_dir(PathBuf::from("/home/user"));
        let dirs = config.backend(BackendId::Cursor);
        assert_eq!(dirs.root_dir, PathBuf::from("/home/user/.cursor"));
        assert_eq!(
            dirs.skills_dir.unwrap(),
            PathBuf::from("/home/user/.cursor/rules")
        );
        assert_eq!(
            dirs.agents_dir.unwrap(),
            PathBuf::from("/home/user/.cursor/agents")
        );
        assert!(dirs.settings_path.is_none());
        assert!(dirs.config_path.is_none());
        assert_eq!(
            dirs.hooks_path.unwrap(),
            PathBuf::from("/home/user/.cursor/hooks.json")
        );
        assert_eq!(
            dirs.mcp_path.unwrap(),
            PathBuf::from("/home/user/.cursor/mcp.json")
        );
    }

    #[test]
    fn test_backend_cursor_project() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        let dirs = config.backend(BackendId::Cursor);
        assert_eq!(dirs.root_dir, PathBuf::from("/home/user/.cursor"));
        assert_eq!(
            dirs.skills_dir.unwrap(),
            PathBuf::from("/projects/foo/.cursor/rules")
        );
        assert_eq!(
            dirs.agents_dir.unwrap(),
            PathBuf::from("/projects/foo/.cursor/agents")
        );
        assert_eq!(
            dirs.hooks_path.unwrap(),
            PathBuf::from("/projects/foo/.cursor/hooks.json")
        );
        assert_eq!(
            dirs.mcp_path.unwrap(),
            PathBuf::from("/projects/foo/.cursor/mcp.json")
        );
    }

    #[test]
    fn test_backend_codex_global() {
        let config = Config::with_home_dir(PathBuf::from("/home/user"));
        let dirs = config.backend(BackendId::Codex);
        assert_eq!(dirs.root_dir, PathBuf::from("/home/user/.codex"));
        assert!(dirs.skills_dir.is_none());
        assert_eq!(
            dirs.agents_dir.unwrap(),
            PathBuf::from("/home/user/.codex/agents")
        );
        assert!(dirs.settings_path.is_none());
        assert_eq!(
            dirs.config_path.unwrap(),
            PathBuf::from("/home/user/.codex/config.toml")
        );
        assert_eq!(
            dirs.hooks_path.unwrap(),
            PathBuf::from("/home/user/.codex/hooks.json")
        );
        assert!(dirs.mcp_path.is_none());
    }

    #[test]
    fn test_backend_codex_project() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        let dirs = config.backend(BackendId::Codex);
        assert_eq!(dirs.root_dir, PathBuf::from("/home/user/.codex"));
        assert_eq!(
            dirs.agents_dir.unwrap(),
            PathBuf::from("/projects/foo/.codex/agents")
        );
        assert_eq!(
            dirs.config_path.unwrap(),
            PathBuf::from("/projects/foo/.codex/config.toml")
        );
        assert_eq!(
            dirs.hooks_path.unwrap(),
            PathBuf::from("/projects/foo/.codex/hooks.json")
        );
    }

    #[test]
    fn test_backend_gemini_global() {
        let config = Config::with_home_dir(PathBuf::from("/home/user"));
        let dirs = config.backend(BackendId::Gemini);
        assert_eq!(dirs.root_dir, PathBuf::from("/home/user/.gemini"));
        assert_eq!(
            dirs.skills_dir.unwrap(),
            PathBuf::from("/home/user/.gemini/skills")
        );
        assert_eq!(
            dirs.agents_dir.unwrap(),
            PathBuf::from("/home/user/.gemini/agents")
        );
        assert_eq!(
            dirs.settings_path.unwrap(),
            PathBuf::from("/home/user/.gemini/settings.json")
        );
        assert!(dirs.config_path.is_none());
        assert!(dirs.hooks_path.is_none());
        assert!(dirs.mcp_path.is_none());
    }

    #[test]
    fn test_backend_gemini_project() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        let dirs = config.backend(BackendId::Gemini);
        assert_eq!(dirs.root_dir, PathBuf::from("/home/user/.gemini"));
        assert_eq!(
            dirs.skills_dir.unwrap(),
            PathBuf::from("/projects/foo/.gemini/skills")
        );
        assert_eq!(
            dirs.agents_dir.unwrap(),
            PathBuf::from("/projects/foo/.gemini/agents")
        );
        assert_eq!(
            dirs.settings_path.unwrap(),
            PathBuf::from("/projects/foo/.gemini/settings.json")
        );
    }

    #[test]
    fn test_backend_agents_global() {
        let config = Config::with_home_dir(PathBuf::from("/home/user"));
        let dirs = config.backend(BackendId::Agents);
        assert_eq!(dirs.root_dir, PathBuf::from("/home/user/.agents"));
        assert_eq!(
            dirs.skills_dir.unwrap(),
            PathBuf::from("/home/user/.agents/skills")
        );
        assert!(dirs.agents_dir.is_none());
        assert!(dirs.settings_path.is_none());
        assert!(dirs.config_path.is_none());
        assert!(dirs.hooks_path.is_none());
        assert!(dirs.mcp_path.is_none());
    }

    #[test]
    fn test_backend_agents_project() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        let dirs = config.backend(BackendId::Agents);
        assert_eq!(dirs.root_dir, PathBuf::from("/projects/foo/.agents"));
        assert_eq!(
            dirs.skills_dir.unwrap(),
            PathBuf::from("/projects/foo/.agents/skills")
        );
    }
}
