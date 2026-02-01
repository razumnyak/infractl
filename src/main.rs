mod cli;
mod config;
mod deploy;
mod error;
mod logging;
mod metrics;
mod server;
mod storage;
mod updater;

use anyhow::Result;
use tracing::info;

const DEFAULT_REPO: &str = "razumnyak/infractl";

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::parse();

    // Handle self-update before full initialization
    if let Some(cli::Commands::SelfUpdate {
        force,
        repo,
        prerelease,
    }) = &cli.command
    {
        // Initialize minimal logging for self-update
        logging::init(&cli)?;

        let repo = repo.as_deref().unwrap_or(DEFAULT_REPO);

        println!("infractl self-update");
        println!("Current version: v{}", env!("CARGO_PKG_VERSION"));
        println!("Repository: {}", repo);
        println!();

        match updater::self_update_standalone(repo, *force, *prerelease).await {
            Ok(result) => {
                if result.requires_restart {
                    println!("Updated: v{} -> {}", result.from_version, result.to_version);
                    println!();
                    println!("Restart the service to apply:");
                    println!("  sudo systemctl restart infractl");
                } else {
                    println!("{}", result.message);
                }
                return Ok(());
            }
            Err(e) => {
                eprintln!("Update failed: {}", e);
                std::process::exit(1);
            }
        }
    }

    logging::init(&cli)?;

    info!(version = env!("CARGO_PKG_VERSION"), "Starting infractl");

    let config = config::load(&cli.config)?;

    info!(mode = ?config.mode, "Configuration loaded");

    server::run(config, cli).await
}
