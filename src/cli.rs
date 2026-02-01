use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "infractl",
    author,
    version,
    about = "Infrastructure monitoring and deployment agent",
    long_about = None
)]
pub struct Cli {
    /// Configuration file path
    #[arg(
        short,
        long,
        default_value = "/etc/infractl/config.yaml",
        env = "INFRACTL_CONFIG"
    )]
    pub config: PathBuf,

    /// Log level (debug, info, warn, error)
    #[arg(short, long, env = "INFRACTL_LOG_LEVEL")]
    pub log_level: Option<String>,

    /// Log format (json, pretty)
    #[arg(long, env = "INFRACTL_LOG_FORMAT")]
    pub log_format: Option<String>,

    /// Run in foreground (don't detach)
    #[arg(short, long)]
    pub foreground: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Start the server (default if no command specified)
    Run,

    /// Validate configuration file
    Validate {
        /// Configuration file to validate
        #[arg(short, long)]
        config: Option<PathBuf>,
    },

    /// Show current version
    Version,

    /// Generate a JWT token (for testing)
    Token {
        /// Token subject (agent name or identifier)
        #[arg(short, long)]
        subject: String,

        /// Token TTL (e.g., "24h", "7d")
        #[arg(short, long, default_value = "24h")]
        ttl: String,
    },

    /// Check health of an agent
    Health {
        /// Agent address (e.g., "10.0.0.2:8111")
        #[arg(short, long)]
        address: String,

        /// Bearer token for authentication
        #[arg(short, long)]
        token: Option<String>,
    },

    /// Trigger a deployment
    Deploy {
        /// Deployment name
        #[arg(short, long)]
        name: String,

        /// Target agent address (for Home mode forwarding)
        #[arg(short, long)]
        target: Option<String>,
    },

    /// Update infractl to the latest version
    SelfUpdate {
        /// Force update even if already on latest version
        #[arg(short, long)]
        force: bool,

        /// GitHub repository (default: from config or razumnyak/infractl)
        #[arg(short, long)]
        repo: Option<String>,

        /// Include pre-release versions
        #[arg(long)]
        prerelease: bool,
    },
}

pub fn parse() -> Cli {
    Cli::parse()
}

impl Cli {
    pub fn effective_log_level(&self) -> &str {
        self.log_level.as_deref().unwrap_or("info")
    }

    pub fn effective_log_format(&self) -> &str {
        self.log_format.as_deref().unwrap_or("json")
    }
}
