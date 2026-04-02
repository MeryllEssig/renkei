mod artifact;
mod backend;
mod cache;
mod cli;
mod config;
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
mod source;
mod uninstall;
mod workspace;

use std::path::{Path, PathBuf};
use std::process;

use clap::Parser;
use owo_colors::OwoColorize;

use backend::{Backend, BackendRegistry};
use cli::{Cli, Commands};
use config::Config;
use manifest::RequestedScope;

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

fn run_install(
    source: &str,
    global: bool,
    tag: Option<&str>,
    force: bool,
    backends: &[&dyn Backend],
) -> error::Result<()> {
    let requested_scope = if global {
        RequestedScope::Global
    } else {
        RequestedScope::Project
    };

    let config = build_config(global)?;

    match source::parse_source(source) {
        source::PackageSource::Local(path_str) => {
            let path = PathBuf::from(&path_str);
            let options = install::InstallOptions {
                force,
                ..install::InstallOptions::local(path_str)
            };
            install_or_workspace(&path, &config, backends, requested_scope, &options)
        }
        source::PackageSource::GitSsh(url) | source::PackageSource::GitUrl(url) => {
            let tmp_dir = git::clone_repo(&url, tag)?;
            let sha = git::resolve_head(tmp_dir.path())?;
            let options = install::InstallOptions {
                force,
                ..install::InstallOptions::git(url, sha, tag.map(String::from))
            };
            install_or_workspace(tmp_dir.path(), &config, backends, requested_scope, &options)
        }
    }
}

fn run_install_from_lockfile(global: bool, backends: &[&dyn Backend]) -> error::Result<()> {
    let config = build_config(global)?;
    lockfile::install_from_lockfile(&config, backends)
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
    let all_backends = registry.detect(&Config::new());

    let result: error::Result<()> = match cli.command {
        Commands::Install {
            source: Some(source),
            global,
            tag,
            force,
        } => run_install(&source, global, tag.as_deref(), force, &all_backends),
        Commands::Install {
            source: None,
            global,
            ..
        } => run_install_from_lockfile(global, &all_backends),
        Commands::List { global } => run_list(global),
        Commands::Doctor { global } => run_doctor(global, &registry),
        Commands::Uninstall { package, global } => run_uninstall(&package, global),
        Commands::Package { bump } => package::run_package(bump),
        Commands::Migrate { path } => migrate::run_migrate(&path),
    };

    if let Err(e) = result {
        eprintln!("{} {}", "Error:".red().bold(), e);
        process::exit(1);
    }
}
