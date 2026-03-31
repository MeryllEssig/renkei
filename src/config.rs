use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub home_dir: PathBuf,
}

impl Config {
    pub fn new() -> Self {
        let home_dir = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));
        Self { home_dir }
    }

    #[allow(dead_code)]
    pub fn with_home_dir(home_dir: PathBuf) -> Self {
        Self { home_dir }
    }

    pub fn renkei_dir(&self) -> PathBuf {
        self.home_dir.join(".renkei")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.renkei_dir().join("cache")
    }

    pub fn install_cache_path(&self) -> PathBuf {
        self.renkei_dir().join("install-cache.json")
    }

    pub fn claude_dir(&self) -> PathBuf {
        self.home_dir.join(".claude")
    }

    pub fn claude_skills_dir(&self) -> PathBuf {
        self.claude_dir().join("skills")
    }

    pub fn claude_agents_dir(&self) -> PathBuf {
        self.claude_dir().join("agents")
    }
}
