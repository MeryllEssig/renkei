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
}
