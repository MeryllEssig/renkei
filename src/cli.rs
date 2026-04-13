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
        /// Install only the specified workspace member(s). Repeatable; comma-separated also accepted.
        #[arg(
            short = 'm',
            long = "member",
            value_delimiter = ',',
            action = clap::ArgAction::Append
        )]
        members: Vec<String>,
        /// Skip the preinstall confirmation prompt and accept all messages.
        #[arg(short = 'y', long = "yes")]
        yes: bool,
        /// Skip the build confirmation prompt for local MCP servers.
        #[arg(long = "allow-build")]
        allow_build: bool,
        /// Symlink the source instead of copying it (live dev mode). Local
        /// MCP folders are linked too; no archive is created and the
        /// install is not recorded in the lockfile.
        #[arg(long = "link")]
        link: bool,
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
    /// Update rk to the latest stable release
    #[command(name = "self-update")]
    SelfUpdate,
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn install_members(args: &[&str]) -> Vec<String> {
        let cli = Cli::try_parse_from(args).expect("parse should succeed");
        match cli.command {
            Commands::Install { members, .. } => members,
            other => panic!("expected Install, got {other:?}"),
        }
    }

    #[test]
    fn install_without_member_flag_yields_empty_vec() {
        assert!(install_members(&["rk", "install", "./pkg"]).is_empty());
    }

    #[test]
    fn install_accepts_repeated_member_flags() {
        let members = install_members(&["rk", "install", "./pkg", "-m", "a", "-m", "b"]);
        assert_eq!(members, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn install_accepts_comma_separated_members() {
        let members = install_members(&["rk", "install", "./pkg", "-m", "a,b,c"]);
        assert_eq!(
            members,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn install_mixes_repeated_and_csv_members() {
        let members = install_members(&["rk", "install", "./pkg", "-m", "a,b", "-m", "c"]);
        assert_eq!(
            members,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn install_member_long_flag_works() {
        let members = install_members(&["rk", "install", "./pkg", "--member", "mr-review"]);
        assert_eq!(members, vec!["mr-review".to_string()]);
    }

    fn install_yes(args: &[&str]) -> bool {
        let cli = Cli::try_parse_from(args).expect("parse should succeed");
        match cli.command {
            Commands::Install { yes, .. } => yes,
            other => panic!("expected Install, got {other:?}"),
        }
    }

    #[test]
    fn install_yes_defaults_false() {
        assert!(!install_yes(&["rk", "install", "./pkg"]));
    }

    #[test]
    fn install_short_yes_flag_sets_true() {
        assert!(install_yes(&["rk", "install", "./pkg", "-y"]));
    }

    #[test]
    fn install_long_yes_flag_sets_true() {
        assert!(install_yes(&["rk", "install", "./pkg", "--yes"]));
    }
}
