use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::error::Result;
use crate::json_file;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserConfig {
    #[serde(default)]
    pub defaults: UserConfigDefaults,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserConfigDefaults {
    /// Preferred backends. When set, only these (intersected with detected) are used.
    pub backends: Option<Vec<String>>,
}

impl UserConfig {
    pub fn load(config: &Config) -> Result<Self> {
        let path = config.renkei_dir().join("config.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let cfg: Self = serde_json::from_str(&content).map_err(|e| {
            crate::error::RenkeiError::CacheError(format!("Failed to parse config.json: {}", e))
        })?;
        Ok(cfg)
    }

    pub fn save(&self, config: &Config) -> Result<()> {
        let path = config.renkei_dir().join("config.json");
        json_file::write_json_pretty(&path, &serde_json::to_value(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn config_for(home: &std::path::Path) -> Config {
        Config::with_home_dir(home.to_path_buf())
    }

    #[test]
    fn test_load_missing_returns_defaults() {
        let dir = tempdir().unwrap();
        let config = config_for(dir.path());
        let user_cfg = UserConfig::load(&config).unwrap();
        assert!(user_cfg.defaults.backends.is_none());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempdir().unwrap();
        let config = config_for(dir.path());

        let user_cfg = UserConfig {
            defaults: UserConfigDefaults {
                backends: Some(vec!["claude".to_string(), "cursor".to_string()]),
            },
        };

        user_cfg.save(&config).unwrap();

        let path = dir.path().join(".renkei/config.json");
        assert!(path.exists());

        let loaded = UserConfig::load(&config).unwrap();
        assert_eq!(
            loaded.defaults.backends,
            Some(vec!["claude".to_string(), "cursor".to_string()])
        );
    }

    #[test]
    fn test_load_with_backends() {
        let dir = tempdir().unwrap();
        let renkei_dir = dir.path().join(".renkei");
        fs::create_dir_all(&renkei_dir).unwrap();
        fs::write(
            renkei_dir.join("config.json"),
            r#"{"defaults":{"backends":["claude","agents"]}}"#,
        )
        .unwrap();

        let config = config_for(dir.path());
        let user_cfg = UserConfig::load(&config).unwrap();
        assert_eq!(
            user_cfg.defaults.backends,
            Some(vec!["claude".to_string(), "agents".to_string()])
        );
    }

    #[test]
    fn test_save_creates_renkei_dir() {
        let dir = tempdir().unwrap();
        let config = config_for(dir.path());

        let user_cfg = UserConfig {
            defaults: UserConfigDefaults {
                backends: Some(vec!["cursor".to_string()]),
            },
        };

        user_cfg.save(&config).unwrap();
        assert!(dir.path().join(".renkei/config.json").exists());
    }

    #[test]
    fn test_load_empty_json_returns_defaults() {
        let dir = tempdir().unwrap();
        let renkei_dir = dir.path().join(".renkei");
        fs::create_dir_all(&renkei_dir).unwrap();
        fs::write(renkei_dir.join("config.json"), "{}").unwrap();

        let config = config_for(dir.path());
        let user_cfg = UserConfig::load(&config).unwrap();
        assert!(user_cfg.defaults.backends.is_none());
    }

    #[test]
    fn test_load_invalid_json_errors() {
        let dir = tempdir().unwrap();
        let renkei_dir = dir.path().join(".renkei");
        fs::create_dir_all(&renkei_dir).unwrap();
        fs::write(renkei_dir.join("config.json"), "not json").unwrap();

        let config = Config::with_home_dir(PathBuf::from(dir.path()));
        let result = UserConfig::load(&config);
        assert!(result.is_err());
    }
}
