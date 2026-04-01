use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "rk",
    version,
    about = "Package manager for AI agentic workflows"
)]
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
        /// Install globally (to ~/.claude/) instead of project-locally
        #[arg(short = 'g', long = "global")]
        global: bool,
        /// Git tag or branch to clone
        #[arg(long = "tag")]
        tag: Option<String>,
        /// Force installation (bypass backend detection)
        #[arg(long = "force")]
        force: bool,
    },
}
