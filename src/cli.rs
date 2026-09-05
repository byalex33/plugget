use crate::minecraft::Platform;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "plugget", version, about = "The package manager for Minecraft server plugins.", color = clap::ColorChoice::Never)]
pub struct Cli {
    #[arg(long, global = true, help = "Suppress normal output")]
    pub quiet: bool,
    #[arg(
        long,
        global = true,
        conflicts_with = "quiet",
        help = "Show HTTP diagnostics on stderr"
    )]
    pub verbose: bool,
    #[arg(long, global = true, help = "Emit one JSON result; never prompt")]
    pub json: bool,
    #[arg(
        short = 'y',
        long,
        global = true,
        help = "Confirm changes and required dependencies"
    )]
    pub yes: bool,
    #[arg(
        long,
        global = true,
        help = "Disable color (output is ASCII by default)"
    )]
    pub no_color: bool,
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Detect a server and create local configuration
    Init {
        #[arg(long)]
        minecraft: Option<String>,
        #[arg(long, value_enum)]
        platform: Option<Platform>,
    },
    /// Search server plugins on Modrinth
    Search {
        query: String,
        #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u8).range(1..=100))]
        limit: u8,
    },
    /// Show a project and its latest compatible version
    Info { plugin: String },
    /// Install a plugin and required dependencies
    Install {
        plugin: String,
        #[arg(long)]
        version: Option<String>,
        #[arg(long)]
        prerelease: bool,
    },
    /// Move a managed plugin JAR to the OS Recycle Bin
    Remove { plugin: String },
    /// List managed and unmanaged plugin JARs (offline)
    List,
    /// Check for compatible updates
    Outdated,
    /// Update one plugin, or all managed plugins when no name is given
    Update {
        #[arg(conflicts_with = "all")]
        plugin: Option<String>,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        prerelease: bool,
    },
    /// Diagnose server, file, metadata and connectivity issues without changes
    Doctor {
        #[arg(long, help = "Skip the connectivity probe")]
        offline: bool,
    },
    /// Show the executable version
    Version,
}
