use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::error::{RenkeiError, Result};

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

    pub fn claude_dir(&self) -> PathBuf {
        self.home_dir.join(".claude")
    }

    fn claude_subdir(&self, name: &str) -> PathBuf {
        match self.project_root {
            Some(ref root) => root.join(".claude").join(name),
            None => self.claude_dir().join(name),
        }
    }

    pub fn claude_skills_dir(&self) -> PathBuf {
        self.claude_subdir("skills")
    }

    pub fn claude_agents_dir(&self) -> PathBuf {
        self.claude_subdir("agents")
    }

    pub fn claude_settings_path(&self) -> PathBuf {
        self.claude_dir().join("settings.json")
    }

    pub fn claude_config_path(&self) -> PathBuf {
        self.home_dir.join(".claude.json")
    }

    pub fn agents_dir(&self) -> PathBuf {
        match self.project_root {
            Some(ref root) => root.join(".agents"),
            None => self.home_dir.join(".agents"),
        }
    }

    pub fn agents_skills_dir(&self) -> PathBuf {
        self.agents_dir().join("skills")
    }

    pub fn cursor_dir(&self) -> PathBuf {
        self.home_dir.join(".cursor")
    }

    fn cursor_subdir(&self, name: &str) -> PathBuf {
        match self.project_root {
            Some(ref root) => root.join(".cursor").join(name),
            None => self.cursor_dir().join(name),
        }
    }

    pub fn cursor_rules_dir(&self) -> PathBuf {
        self.cursor_subdir("rules")
    }

    pub fn cursor_agents_dir(&self) -> PathBuf {
        self.cursor_subdir("agents")
    }

    pub fn cursor_hooks_path(&self) -> PathBuf {
        self.cursor_subdir("hooks.json")
    }

    pub fn cursor_mcp_path(&self) -> PathBuf {
        self.cursor_subdir("mcp.json")
    }

    pub fn codex_dir(&self) -> PathBuf {
        self.home_dir.join(".codex")
    }

    fn codex_subdir(&self, name: &str) -> PathBuf {
        match self.project_root {
            Some(ref root) => root.join(".codex").join(name),
            None => self.codex_dir().join(name),
        }
    }

    pub fn codex_agents_dir(&self) -> PathBuf {
        self.codex_subdir("agents")
    }

    pub fn codex_hooks_path(&self) -> PathBuf {
        self.codex_subdir("hooks.json")
    }

    pub fn codex_config_path(&self) -> PathBuf {
        self.codex_subdir("config.toml")
    }

    pub fn gemini_dir(&self) -> PathBuf {
        self.home_dir.join(".gemini")
    }

    fn gemini_subdir(&self, name: &str) -> PathBuf {
        match self.project_root {
            Some(ref root) => root.join(".gemini").join(name),
            None => self.gemini_dir().join(name),
        }
    }

    pub fn gemini_skills_dir(&self) -> PathBuf {
        self.gemini_subdir("skills")
    }

    pub fn gemini_agents_dir(&self) -> PathBuf {
        self.gemini_subdir("agents")
    }

    pub fn gemini_settings_path(&self) -> PathBuf {
        self.gemini_subdir("settings.json")
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
    fn test_for_project_skills_dir() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        assert_eq!(
            config.claude_skills_dir(),
            PathBuf::from("/projects/foo/.claude/skills")
        );
    }

    #[test]
    fn test_for_project_agents_dir() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        assert_eq!(
            config.claude_agents_dir(),
            PathBuf::from("/projects/foo/.claude/agents")
        );
    }

    #[test]
    fn test_for_project_settings_path_stays_global() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        assert_eq!(
            config.claude_settings_path(),
            PathBuf::from("/home/user/.claude/settings.json")
        );
    }

    #[test]
    fn test_for_project_config_path_stays_global() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        assert_eq!(
            config.claude_config_path(),
            PathBuf::from("/home/user/.claude.json")
        );
    }

    #[test]
    fn test_global_config_skills_dir() {
        let config = Config::with_home_dir(PathBuf::from("/home/user"));
        assert_eq!(
            config.claude_skills_dir(),
            PathBuf::from("/home/user/.claude/skills")
        );
    }

    #[test]
    fn test_global_config_agents_dir() {
        let config = Config::with_home_dir(PathBuf::from("/home/user"));
        assert_eq!(
            config.claude_agents_dir(),
            PathBuf::from("/home/user/.claude/agents")
        );
    }

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
    fn test_agents_dir_global() {
        let config = Config::with_home_dir(PathBuf::from("/home/user"));
        assert_eq!(config.agents_dir(), PathBuf::from("/home/user/.agents"));
    }

    #[test]
    fn test_agents_dir_project() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        assert_eq!(config.agents_dir(), PathBuf::from("/projects/foo/.agents"));
    }

    #[test]
    fn test_agents_skills_dir_global() {
        let config = Config::with_home_dir(PathBuf::from("/home/user"));
        assert_eq!(
            config.agents_skills_dir(),
            PathBuf::from("/home/user/.agents/skills")
        );
    }

    #[test]
    fn test_agents_skills_dir_project() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        assert_eq!(
            config.agents_skills_dir(),
            PathBuf::from("/projects/foo/.agents/skills")
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

    #[test]
    fn test_cursor_dir_always_global() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        assert_eq!(config.cursor_dir(), PathBuf::from("/home/user/.cursor"));
    }

    #[test]
    fn test_cursor_rules_dir_project() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        assert_eq!(
            config.cursor_rules_dir(),
            PathBuf::from("/projects/foo/.cursor/rules")
        );
    }

    #[test]
    fn test_cursor_rules_dir_global() {
        let config = Config::with_home_dir(PathBuf::from("/home/user"));
        assert_eq!(
            config.cursor_rules_dir(),
            PathBuf::from("/home/user/.cursor/rules")
        );
    }

    #[test]
    fn test_cursor_hooks_path_project() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        assert_eq!(
            config.cursor_hooks_path(),
            PathBuf::from("/projects/foo/.cursor/hooks.json")
        );
    }

    #[test]
    fn test_cursor_mcp_path_global() {
        let config = Config::with_home_dir(PathBuf::from("/home/user"));
        assert_eq!(
            config.cursor_mcp_path(),
            PathBuf::from("/home/user/.cursor/mcp.json")
        );
    }

    #[test]
    fn test_codex_dir_always_global() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        assert_eq!(config.codex_dir(), PathBuf::from("/home/user/.codex"));
    }

    #[test]
    fn test_codex_agents_dir_project() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        assert_eq!(
            config.codex_agents_dir(),
            PathBuf::from("/projects/foo/.codex/agents")
        );
    }

    #[test]
    fn test_codex_config_path_project() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        assert_eq!(
            config.codex_config_path(),
            PathBuf::from("/projects/foo/.codex/config.toml")
        );
    }

    #[test]
    fn test_gemini_dir_always_global() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        assert_eq!(config.gemini_dir(), PathBuf::from("/home/user/.gemini"));
    }

    #[test]
    fn test_gemini_skills_dir_project() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        assert_eq!(
            config.gemini_skills_dir(),
            PathBuf::from("/projects/foo/.gemini/skills")
        );
    }

    #[test]
    fn test_gemini_settings_path_project() {
        let config =
            Config::for_project(PathBuf::from("/home/user"), PathBuf::from("/projects/foo"));
        assert_eq!(
            config.gemini_settings_path(),
            PathBuf::from("/projects/foo/.gemini/settings.json")
        );
    }

    #[test]
    fn test_gemini_settings_path_global() {
        let config = Config::with_home_dir(PathBuf::from("/home/user"));
        assert_eq!(
            config.gemini_settings_path(),
            PathBuf::from("/home/user/.gemini/settings.json")
        );
    }
}
