mod artifact;
mod backend;
mod cache;
mod cli;
mod config;
mod error;
mod hook;
mod install;
mod install_cache;
mod manifest;

use clap::Parser;
use cli::{Cli, Commands};
use config::Config;
use owo_colors::OwoColorize;
use std::path::PathBuf;
use std::process;

use backend::claude::ClaudeBackend;

fn main() {
    let cli = Cli::parse();
    let config = Config::new();
    let backend = ClaudeBackend;

    let result: error::Result<()> = match cli.command {
        Commands::Install { source } => {
            let path = PathBuf::from(&source);
            install::install_local(&path, &config, &backend)
        }
    };

    if let Err(e) = result {
        eprintln!("{} {}", "Error:".red().bold(), e);
        process::exit(1);
    }
}
