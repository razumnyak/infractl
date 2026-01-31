mod cli;
mod config;
mod deploy;
mod error;
mod logging;
mod metrics;
mod server;
mod storage;

use anyhow::Result;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::parse();

    logging::init(&cli)?;

    info!(version = env!("CARGO_PKG_VERSION"), "Starting infractl");

    let config = config::load(&cli.config)?;

    info!(mode = ?config.mode, "Configuration loaded");

    server::run(config, cli).await
}
