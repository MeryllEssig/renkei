mod artifact;
mod backend;
mod cache;
mod cli;
mod config;
mod config_cmd;
mod conflict;
mod doctor;
mod env_check;
mod error;
mod frontmatter;
mod git;
mod hook;
mod install;
mod install_cache;
mod json_file;
mod list;
mod lockfile;
mod manifest;
mod mcp;
mod migrate;
mod package;
mod package_store;
mod source;
mod uninstall;
mod user_config;
mod workspace;

use std::path::{Path, PathBuf};
use std::process;

use clap::Parser;
use owo_colors::OwoColorize;

use backend::{Backend, BackendRegistry};
use cli::{Cli, Commands, ConfigAction};
use config::Config;
use error::RenkeiError;
use manifest::RequestedScope;
use user_config::UserConfig;

fn build_config(global: bool) -> error::Result<Config> {
    let home_dir = Config::default_home_dir();
    if global {
        Ok(Config::with_home_dir(home_dir))
    } else {
        let project_root = config::detect_project_root()?;
        Ok(Config::for_project(home_dir, project_root))
    }
}

/// Dispatch to workspace or single-package install based on the manifest.
fn install_or_workspace(
    package_dir: &Path,
    config: &Config,
    backends: &[&dyn Backend],
    requested_scope: RequestedScope,
    options: &install::InstallOptions,
) -> error::Result<()> {
    if let Some(members) = manifest::try_load_workspace(package_dir) {
        workspace::install_workspace(
            package_dir,
            &members,
            config,
            backends,
            requested_scope,
            options,
        )
    } else {
        install::install_local(package_dir, config, backends, requested_scope, options)
    }
}

/// Resolve which backends to use for an install, applying the override/config/autodetect cascade.
fn resolve_backends<'a>(
    registry: &'a BackendRegistry,
    config: &Config,
    backend_override: Option<&str>,
) -> error::Result<Vec<&'a dyn Backend>> {
    if let Some(name) = backend_override {
        // --backend flag: bypass detection and manifest, use named backend directly
        let b = registry
            .get(name)
            .ok_or_else(|| RenkeiError::BackendNotFound(name.to_string()))?;
        return Ok(vec![b]);
    }

    let user_cfg = UserConfig::load(config).unwrap_or_default();
    if let Some(cfg_names) = user_cfg.defaults.backends {
        // User config: filter detected backends by config list
        let detected = registry.detect(config);
        let filtered: Vec<&dyn Backend> = detected
            .into_iter()
            .filter(|b| cfg_names.iter().any(|n| n == b.name()))
            .collect();
        return Ok(filtered);
    }

    // Default: auto-detect
    Ok(registry.detect(config))
}

fn run_install(
    source: &str,
    global: bool,
    tag: Option<&str>,
    force: bool,
    backend_override: Option<&str>,
    registry: &BackendRegistry,
) -> error::Result<()> {
    let requested_scope = if global {
        RequestedScope::Global
    } else {
        RequestedScope::Project
    };

    let config = build_config(global)?;
    let backends = resolve_backends(registry, &config, backend_override)?;

    match source::parse_source(source) {
        source::PackageSource::Local(path_str) => {
            let path = PathBuf::from(&path_str);
            let options = install::InstallOptions {
                force: force || backend_override.is_some(),
                ..install::InstallOptions::local(path_str)
            };
            install_or_workspace(&path, &config, &backends, requested_scope, &options)
        }
        source::PackageSource::GitSsh(url) | source::PackageSource::GitUrl(url) => {
            let tmp_dir = git::clone_repo(&url, tag)?;
            let sha = git::resolve_head(tmp_dir.path())?;
            let options = install::InstallOptions {
                force: force || backend_override.is_some(),
                ..install::InstallOptions::git(url, sha, tag.map(String::from))
            };
            install_or_workspace(
                tmp_dir.path(),
                &config,
                &backends,
                requested_scope,
                &options,
            )
        }
    }
}

fn run_install_from_lockfile(
    global: bool,
    backend_override: Option<&str>,
    registry: &BackendRegistry,
) -> error::Result<()> {
    let config = build_config(global)?;
    let backends = resolve_backends(registry, &config, backend_override)?;
    lockfile::install_from_lockfile(&config, &backends)
}

fn run_uninstall(package: &str, global: bool) -> error::Result<()> {
    let config = build_config(global)?;
    uninstall::run_uninstall(package, &config)
}

fn run_list(global: bool) -> error::Result<()> {
    let config = build_config(global)?;
    list::run_list(&config, global)
}

fn run_doctor(global: bool, registry: &BackendRegistry) -> error::Result<()> {
    let config = build_config(global)?;
    let healthy = doctor::run_doctor(&config, global, registry)?;
    if !healthy {
        process::exit(1);
    }
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let registry = BackendRegistry::all();

    let result: error::Result<()> = match cli.command {
        Commands::Install {
            source: Some(source),
            global,
            tag,
            force,
            backend,
        } => run_install(
            &source,
            global,
            tag.as_deref(),
            force,
            backend.as_deref(),
            &registry,
        ),
        Commands::Install {
            source: None,
            global,
            backend,
            ..
        } => run_install_from_lockfile(global, backend.as_deref(), &registry),
        Commands::List { global } => run_list(global),
        Commands::Doctor { global } => run_doctor(global, &registry),
        Commands::Uninstall { package, global } => run_uninstall(&package, global),
        Commands::Package { bump } => package::run_package(bump),
        Commands::Migrate { path } => migrate::run_migrate(&path),
        Commands::Config { action: None } => {
            let config = Config::new();
            config_cmd::run_config_interactive(&config, &registry)
        }
        Commands::Config {
            action: Some(ConfigAction::Set { key, value }),
        } => {
            let config = Config::new();
            config_cmd::run_config_set(&key, &value, &config)
        }
        Commands::Config {
            action: Some(ConfigAction::Get { key }),
        } => {
            let config = Config::new();
            config_cmd::run_config_get(&key, &config)
        }
        Commands::Config {
            action: Some(ConfigAction::List),
        } => {
            let config = Config::new();
            config_cmd::run_config_list(&config)
        }
    };

    if let Err(e) = result {
        eprintln!("{} {}", "Error:".red().bold(), e);
        process::exit(1);
    }
}
