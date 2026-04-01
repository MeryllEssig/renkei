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
mod lockfile;
mod json_file;
mod list;
mod manifest;
mod package;
mod mcp;
mod source;
mod uninstall;
mod workspace;

use clap::Parser;
use cli::{Cli, Commands};
use config::Config;
use manifest::RequestedScope;
use owo_colors::OwoColorize;
use std::path::PathBuf;
use std::process;

use backend::claude::ClaudeBackend;
use backend::Backend;

fn build_config(global: bool) -> error::Result<Config> {
    let home_dir = Config::default_home_dir();
    if global {
        Ok(Config::with_home_dir(home_dir))
    } else {
        let project_root = config::detect_project_root()?;
        Ok(Config::for_project(home_dir, project_root))
    }
}

fn run_install(
    source: &str,
    global: bool,
    tag: Option<&str>,
    force: bool,
    backend: &dyn Backend,
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
            if let Some(members) = manifest::try_load_workspace(&path) {
                let ws_options = workspace::WorkspaceInstallOptions {
                    force,
                    source_kind: install::SourceKind::Local,
                    source_url: path_str,
                    resolved: None,
                    tag: None,
                };
                workspace::install_workspace(
                    &path,
                    &members,
                    &config,
                    backend,
                    requested_scope,
                    &ws_options,
                )
            } else {
                let options = install::InstallOptions {
                    force,
                    ..install::InstallOptions::local(path_str)
                };
                install::install_local(&path, &config, backend, requested_scope, &options)
            }
        }
        source::PackageSource::GitSsh(url) | source::PackageSource::GitUrl(url) => {
            let tmp_dir = git::clone_repo(&url, tag)?;
            let sha = git::resolve_head(tmp_dir.path())?;
            if let Some(members) = manifest::try_load_workspace(tmp_dir.path()) {
                let ws_options = workspace::WorkspaceInstallOptions {
                    force,
                    source_kind: install::SourceKind::Git,
                    source_url: url,
                    resolved: Some(sha),
                    tag: tag.map(String::from),
                };
                workspace::install_workspace(
                    tmp_dir.path(),
                    &members,
                    &config,
                    backend,
                    requested_scope,
                    &ws_options,
                )
            } else {
                let options = install::InstallOptions {
                    force,
                    ..install::InstallOptions::git(url, sha, tag.map(String::from))
                };
                install::install_local(
                    tmp_dir.path(),
                    &config,
                    backend,
                    requested_scope,
                    &options,
                )
            }
        }
    }
}

fn run_install_from_lockfile(global: bool, backend: &dyn Backend) -> error::Result<()> {
    let config = build_config(global)?;
    lockfile::install_from_lockfile(&config, backend)
}

fn run_uninstall(package: &str, global: bool) -> error::Result<()> {
    let config = build_config(global)?;
    uninstall::run_uninstall(package, &config)
}

fn run_list(global: bool) -> error::Result<()> {
    let config = build_config(global)?;
    list::run_list(&config, global)
}

fn run_doctor(global: bool, backend: &dyn Backend) -> error::Result<()> {
    let config = build_config(global)?;
    let healthy = doctor::run_doctor(&config, global, backend)?;
    if !healthy {
        process::exit(1);
    }
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let backend = ClaudeBackend;

    let result: error::Result<()> = match cli.command {
        Commands::Install {
            source: Some(source),
            global,
            tag,
            force,
        } => run_install(&source, global, tag.as_deref(), force, &backend),
        Commands::Install {
            source: None,
            global,
            ..
        } => run_install_from_lockfile(global, &backend),
        Commands::List { global } => run_list(global),
        Commands::Doctor { global } => run_doctor(global, &backend),
        Commands::Uninstall { package, global } => run_uninstall(&package, global),
        Commands::Package { bump } => package::run_package(bump),
    };

    if let Err(e) = result {
        eprintln!("{} {}", "Error:".red().bold(), e);
        process::exit(1);
    }
}
