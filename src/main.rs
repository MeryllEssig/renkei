mod artifact;
mod backend;
mod cache;
mod cli;
mod config;
mod error;
mod install_cache;
mod manifest;

use clap::Parser;
use cli::{Cli, Commands};
use owo_colors::OwoColorize;
use std::path::PathBuf;
use std::process;

fn main() {
    let cli = Cli::parse();

    let result: error::Result<()> = match cli.command {
        Commands::Install { source } => {
            let path = PathBuf::from(&source);
            if path.exists() {
                eprintln!("Local install not yet wired — coming in step 10");
                Ok(())
            } else {
                eprintln!("{} Path does not exist: {}", "Error:".red().bold(), source);
                process::exit(1);
            }
        }
    };

    if let Err(e) = result {
        eprintln!("{} {}", "Error:".red().bold(), e);
        process::exit(1);
    }
}
