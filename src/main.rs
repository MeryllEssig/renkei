mod artifact;
mod backend;
mod cache;
mod cli;
mod config;
mod error;
mod env_check;
mod hook;
mod install;
mod install_cache;
mod json_file;
mod manifest;
mod mcp;

use clap::Parser;
use cli::{Cli, Commands};
use config::Config;
use manifest::RequestedScope;
use owo_colors::OwoColorize;
use std::path::PathBuf;
use std::process;

use backend::claude::ClaudeBackend;
use backend::Backend;

fn run_install(source: &str, global: bool, backend: &dyn Backend) -> error::Result<()> {
    let path = PathBuf::from(source);

    let requested_scope = if global {
        RequestedScope::Global
    } else {
        RequestedScope::Project
    };

    let home_dir = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"));

    let config = if global {
        Config::with_home_dir(home_dir)
    } else {
        let project_root = config::detect_project_root()?;
        Config::for_project(home_dir, project_root)
    };

    install::install_local(&path, &config, backend, requested_scope)
}

fn main() {
    let cli = Cli::parse();
    let backend = ClaudeBackend;

    let result: error::Result<()> = match cli.command {
        Commands::Install { source, global } => run_install(&source, global, &backend),
    };

    if let Err(e) = result {
        eprintln!("{} {}", "Error:".red().bold(), e);
        process::exit(1);
    }
}
