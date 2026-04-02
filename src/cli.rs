use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, ValueEnum)]
pub enum BumpLevel {
    Patch,
    Minor,
    Major,
}

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
        /// Force a specific backend, bypassing manifest and config (e.g. --backend cursor)
        #[arg(long = "backend")]
        backend: Option<String>,
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
    /// Create a distributable .tar.gz archive of the current package
    Package {
        /// Bump the version before packaging (patch, minor, major)
        #[arg(long)]
        bump: Option<BumpLevel>,
    },
    /// Migrate an existing directory into a valid Renkei package
    Migrate {
        /// Path to the directory to migrate
        path: String,
    },
    /// Manage renkei configuration
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Set a config value (e.g. defaults.backends claude,cursor)
    Set {
        /// Dot-notation key (e.g. defaults.backends)
        key: String,
        /// Value to set (comma-separated for lists, e.g. claude,cursor)
        value: String,
    },
    /// Get a config value
    Get {
        /// Dot-notation key (e.g. defaults.backends)
        key: String,
    },
    /// List all configuration
    List,
}
