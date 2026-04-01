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
        /// Path to a local package directory, or a git URL. Omit to install from rk.lock.
        source: Option<String>,
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
    /// Uninstall a workflow package
    Uninstall {
        /// Package name (e.g., @scope/name)
        package: String,
        /// Uninstall from global scope instead of project scope
        #[arg(short = 'g', long = "global")]
        global: bool,
    },
    /// List installed packages
    List {
        /// List globally installed packages instead of project-scoped
        #[arg(short = 'g', long = "global")]
        global: bool,
    },
    /// Run health checks on installed packages
    Doctor {
        /// Check globally installed packages instead of project-scoped
        #[arg(short = 'g', long = "global")]
        global: bool,
    },
}
