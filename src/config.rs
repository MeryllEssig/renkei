use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub home_dir: PathBuf,
    pub project_root: Option<PathBuf>,
}

impl Config {
    pub fn new() -> Self {
        let home_dir = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));
        Self {
            home_dir,
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
        self.renkei_dir().join("install-cache.json")
    }

    pub fn claude_dir(&self) -> PathBuf {
        self.home_dir.join(".claude")
    }

    pub fn claude_skills_dir(&self) -> PathBuf {
        match self.project_root {
            Some(ref root) => root.join(".claude").join("skills"),
            None => self.claude_dir().join("skills"),
        }
    }

    pub fn claude_agents_dir(&self) -> PathBuf {
        match self.project_root {
            Some(ref root) => root.join(".claude").join("agents"),
            None => self.claude_dir().join("agents"),
        }
    }

    pub fn claude_settings_path(&self) -> PathBuf {
        self.claude_dir().join("settings.json")
    }

    pub fn claude_config_path(&self) -> PathBuf {
        self.home_dir.join(".claude.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_for_project_skills_dir() {
        let config = Config::for_project(
            PathBuf::from("/home/user"),
            PathBuf::from("/projects/foo"),
        );
        assert_eq!(
            config.claude_skills_dir(),
            PathBuf::from("/projects/foo/.claude/skills")
        );
    }

    #[test]
    fn test_for_project_agents_dir() {
        let config = Config::for_project(
            PathBuf::from("/home/user"),
            PathBuf::from("/projects/foo"),
        );
        assert_eq!(
            config.claude_agents_dir(),
            PathBuf::from("/projects/foo/.claude/agents")
        );
    }

    #[test]
    fn test_for_project_settings_path_stays_global() {
        let config = Config::for_project(
            PathBuf::from("/home/user"),
            PathBuf::from("/projects/foo"),
        );
        assert_eq!(
            config.claude_settings_path(),
            PathBuf::from("/home/user/.claude/settings.json")
        );
    }

    #[test]
    fn test_for_project_config_path_stays_global() {
        let config = Config::for_project(
            PathBuf::from("/home/user"),
            PathBuf::from("/projects/foo"),
        );
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
}
