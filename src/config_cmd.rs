use crate::backend::{BackendRegistry, ALL_BACKEND_NAMES};
use crate::config::Config;
use crate::error::{RenkeiError, Result};
use crate::user_config::{UserConfig, UserConfigDefaults};

pub fn run_config_interactive(config: &Config, registry: &BackendRegistry) -> Result<()> {
    let user_cfg = UserConfig::load(config).unwrap_or_default();
    let detect_config = Config::new();
    let detected_names: Vec<&str> = registry
        .detect(&detect_config)
        .iter()
        .map(|b| b.name())
        .collect();

    let current_backends: Vec<&str> = user_cfg
        .defaults
        .backends
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|s| s.as_str())
        .collect();

    let options: Vec<&str> = ALL_BACKEND_NAMES.to_vec();

    // Pre-select: current config if set, otherwise detected backends
    let defaults: Vec<&str> = if !current_backends.is_empty() {
        current_backends
    } else {
        detected_names.iter().copied().collect()
    };

    let default_indices: Vec<usize> = options
        .iter()
        .enumerate()
        .filter(|(_, name)| defaults.contains(name))
        .map(|(i, _)| i)
        .collect();

    let selected = inquire::MultiSelect::new("Select default backends:", options)
        .with_default(&default_indices)
        .prompt()
        .map_err(|e| RenkeiError::DeploymentFailed(format!("Prompt cancelled: {e}")))?;

    let new_cfg = UserConfig {
        defaults: UserConfigDefaults {
            backends: if selected.is_empty() {
                None
            } else {
                Some(selected.iter().map(|s| s.to_string()).collect())
            },
        },
    };

    new_cfg.save(config)?;
    println!("Configuration saved to ~/.renkei/config.json");
    Ok(())
}

pub fn run_config_set(key: &str, value: &str, config: &Config) -> Result<()> {
    match key {
        "defaults.backends" => {
            let names: Vec<String> = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            // Validate each name
            for name in &names {
                if !ALL_BACKEND_NAMES.contains(&name.as_str()) {
                    return Err(RenkeiError::BackendNotFound(name.clone()));
                }
            }

            let mut cfg = UserConfig::load(config).unwrap_or_default();
            cfg.defaults.backends = if names.is_empty() { None } else { Some(names) };
            cfg.save(config)?;
            println!("Set defaults.backends = {}", value);
            Ok(())
        }
        _ => Err(RenkeiError::InvalidManifest(format!(
            "Unknown config key '{}'. Supported keys: defaults.backends",
            key
        ))),
    }
}

pub fn run_config_get(key: &str, config: &Config) -> Result<()> {
    let cfg = UserConfig::load(config).unwrap_or_default();
    match key {
        "defaults.backends" => match &cfg.defaults.backends {
            Some(backends) => println!("{}", backends.join(",")),
            None => println!("(not set)"),
        },
        _ => {
            return Err(RenkeiError::InvalidManifest(format!(
                "Unknown config key '{}'. Supported keys: defaults.backends",
                key
            )))
        }
    }
    Ok(())
}

pub fn run_config_list(config: &Config) -> Result<()> {
    let cfg = UserConfig::load(config).unwrap_or_default();
    let json = serde_json::to_string_pretty(&cfg)?;
    println!("{}", json);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn config_for(home: &std::path::Path) -> Config {
        Config::with_home_dir(home.to_path_buf())
    }

    #[test]
    fn test_config_set_backends() {
        let dir = tempdir().unwrap();
        let config = config_for(dir.path());

        run_config_set("defaults.backends", "claude,cursor", &config).unwrap();

        let cfg = UserConfig::load(&config).unwrap();
        assert_eq!(
            cfg.defaults.backends,
            Some(vec!["claude".to_string(), "cursor".to_string()])
        );
    }

    #[test]
    fn test_config_get_backends() {
        let dir = tempdir().unwrap();
        let renkei_dir = dir.path().join(".renkei");
        fs::create_dir_all(&renkei_dir).unwrap();
        fs::write(
            renkei_dir.join("config.json"),
            r#"{"defaults":{"backends":["claude","agents"]}}"#,
        )
        .unwrap();

        let config = config_for(dir.path());
        // Just ensure it runs without error (output goes to stdout)
        run_config_get("defaults.backends", &config).unwrap();
    }

    #[test]
    fn test_config_list() {
        let dir = tempdir().unwrap();
        let config = config_for(dir.path());
        // Should work on empty config
        run_config_list(&config).unwrap();
    }

    #[test]
    fn test_config_set_invalid_backend_errors() {
        let dir = tempdir().unwrap();
        let config = config_for(dir.path());

        let result = run_config_set("defaults.backends", "claude,unknown-backend", &config);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("unknown-backend"));
    }

    #[test]
    fn test_config_set_invalid_key_errors() {
        let dir = tempdir().unwrap();
        let config = config_for(dir.path());

        let result = run_config_set("unknown.key", "value", &config);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("unknown.key"));
    }

    #[test]
    fn test_config_get_unknown_key_errors() {
        let dir = tempdir().unwrap();
        let config = config_for(dir.path());

        let result = run_config_get("unknown.key", &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_set_get_roundtrip() {
        let dir = tempdir().unwrap();
        let config = config_for(dir.path());

        run_config_set("defaults.backends", "claude,agents", &config).unwrap();

        let cfg = UserConfig::load(&config).unwrap();
        assert_eq!(
            cfg.defaults.backends,
            Some(vec!["claude".to_string(), "agents".to_string()])
        );
    }
}
