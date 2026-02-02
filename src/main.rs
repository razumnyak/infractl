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

    // Handle CLI commands that don't require starting the server
    match &cli.command {
        // Version command
        Some(cli::Commands::Version) => {
            println!("infractl v{}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }

        // Validate config command
        Some(cli::Commands::Validate { config: cfg_path }) => {
            let path = cfg_path.as_ref().unwrap_or(&cli.config);
            match config::load(path) {
                Ok(cfg) => {
                    println!("Configuration is valid");
                    println!("  Mode: {:?}", cfg.mode);
                    println!("  Port: {}", cfg.server.port);
                    println!("  Deployments: {}", cfg.modules.deploy.deployments.len());
                }
                Err(e) => {
                    eprintln!("Configuration error: {}", e);
                    std::process::exit(1);
                }
            }
            return Ok(());
        }

        // Token generation command
        Some(cli::Commands::Token { subject, ttl }) => {
            let config = config::load(&cli.config)?;
            let ttl_hours = server::auth::parse_ttl_to_hours(ttl);
            let jwt_manager = server::auth::JwtManager::new(&config.auth.jwt_secret);

            match jwt_manager.generate_token(subject, ttl_hours) {
                Ok(token) => {
                    println!("{}", token);
                }
                Err(e) => {
                    eprintln!("Failed to generate token: {}", e);
                    std::process::exit(1);
                }
            }
            return Ok(());
        }

        // Health check command
        Some(cli::Commands::Health { address, token }) => {
            let url = if address.starts_with("http") {
                format!("{}/health", address)
            } else {
                format!("http://{}/health", address)
            };

            let client = reqwest::Client::new();
            let mut req = client.get(&url);

            // Use provided token, or generate from config if not provided
            if let Some(t) = token {
                req = req.header("Authorization", format!("Bearer {}", t));
            } else if let Ok(config) = config::load(&cli.config) {
                let jwt_manager = server::auth::JwtManager::new(&config.auth.jwt_secret);
                if let Ok(t) = jwt_manager.generate_token("cli", 1) {
                    req = req.header("Authorization", format!("Bearer {}", t));
                }
            }

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    if status.is_success() {
                        println!("{}", body);
                    } else {
                        eprintln!("Health check failed ({}): {}", status, body);
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to connect: {}", e);
                    std::process::exit(1);
                }
            }
            return Ok(());
        }

        // Deploy command (connect to running service)
        Some(cli::Commands::Deploy { name, target }) => {
            let config = config::load(&cli.config)?;
            let port = config.server.port;
            let base_url = format!("http://127.0.0.1:{}", port);

            // Generate token from config
            let jwt_manager = server::auth::JwtManager::new(&config.auth.jwt_secret);
            let token = jwt_manager
                .generate_token("cli", 1)
                .map_err(|e| anyhow::anyhow!("Failed to generate token: {}", e))?;

            let url = match target {
                Some(addr) => format!("{}/api/agents/{}/deploy/{}", base_url, addr, name),
                None => format!("{}/webhook/deploy/{}", base_url, name),
            };

            println!("Triggering deployment: {}", name);

            let client = reqwest::Client::new();
            match client
                .post(&url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    if status.is_success() {
                        println!("Deployment triggered successfully");
                        println!("{}", body);
                    } else {
                        eprintln!("Deployment failed ({}): {}", status, body);
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to connect to infractl service: {}", e);
                    eprintln!("Is the service running? Check: systemctl status infractl");
                    std::process::exit(1);
                }
            }
            return Ok(());
        }

        // Run or no command - continue to start server
        Some(cli::Commands::Run) | None => {}

        // SelfUpdate is handled earlier
        Some(cli::Commands::SelfUpdate { .. }) => unreachable!(),
    }

    logging::init(&cli)?;

    info!(version = env!("CARGO_PKG_VERSION"), "Starting infractl");

    let config = config::load(&cli.config)?;

    info!(mode = ?config.mode, "Configuration loaded");

    server::run(config, cli).await
}
