use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "rk", version, about = "Package manager for AI agentic workflows")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Install a workflow package from a local path or git URL
    Install {
        /// Path to a local package directory, or a git URL
        source: String,
    },
}
