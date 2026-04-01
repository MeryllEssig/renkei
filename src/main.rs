mod artifact;
mod backend;
mod cache;
mod cli;
mod config;
mod env_check;
mod error;
mod git;
mod hook;
mod install;
mod install_cache;
mod json_file;
mod manifest;
mod mcp;
mod source;

use clap::Parser;
use cli::{Cli, Commands};
use config::Config;
use manifest::RequestedScope;
use owo_colors::OwoColorize;
use std::path::PathBuf;
use std::process;

use backend::claude::ClaudeBackend;
use backend::Backend;

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

    let home_dir = Config::default_home_dir();

    let config = if global {
        Config::with_home_dir(home_dir)
    } else {
        let project_root = config::detect_project_root()?;
        Config::for_project(home_dir, project_root)
    };

    match source::parse_source(source) {
        source::PackageSource::Local(path_str) => {
            let path = PathBuf::from(&path_str);
            let options = install::InstallOptions {
                force,
                ..install::InstallOptions::local(path_str)
            };
            install::install_local(&path, &config, backend, requested_scope, &options)
        }
        source::PackageSource::GitSsh(url) | source::PackageSource::GitHttps(url) => {
            let tmp_dir = git::clone_repo(&url, tag)?;
            let sha = git::resolve_head(tmp_dir.path())?;
            let options = install::InstallOptions {
                force,
                ..install::InstallOptions::git(url, sha, tag.map(String::from))
            };
            install::install_local(tmp_dir.path(), &config, backend, requested_scope, &options)
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let backend = ClaudeBackend;

    let result: error::Result<()> = match cli.command {
        Commands::Install {
            source,
            global,
            tag,
            force,
        } => run_install(&source, global, tag.as_deref(), force, &backend),
    };

    if let Err(e) = result {
        eprintln!("{} {}", "Error:".red().bold(), e);
        process::exit(1);
    }
}
