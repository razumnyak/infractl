mod app_state;
mod assets;
pub mod auth;
pub mod middleware;
mod routes;
mod shutdown;

pub use app_state::AppState;

use crate::cli::Cli;
use crate::config::{Config, Mode};
use crate::deploy::{self, DeployExecutor};
use crate::storage::aggregation;
use crate::storage::{self, parse_retention_days};
use anyhow::Result;
use axum::{middleware as axum_mw, Router};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

pub async fn run(config: Config, _cli: Cli) -> Result<()> {
    // Initialize database for Home mode
    let state = if config.mode == Mode::Home && config.modules.storage.enabled {
        let db = storage::init(&config).await?;

        // Start background tasks
        let db_clone = db.clone();
        tokio::spawn(aggregation::aggregation_task(db_clone, 3600)); // hourly

        let db_clone = db.clone();
        tokio::spawn(aggregation::daily_aggregation_task(db_clone));

        // Parse retention settings and start cleanup task
        let raw_days = parse_retention_days(&config.modules.storage.retention.raw_data);
        let hourly_days = parse_retention_days(&config.modules.storage.retention.hourly_data);
        let daily_days = parse_retention_days(&config.modules.storage.retention.daily_data);

        let db_clone = db.clone();
        tokio::spawn(aggregation::retention_task(
            db_clone,
            raw_days,
            hourly_days,
            daily_days,
        ));

        AppState::with_database(config.clone(), db)
    } else {
        AppState::new(config.clone())
    };

    // Start deployment worker if enabled
    if config.modules.deploy.enabled {
        if let Some(ref queue) = state.deploy_queue {
            let queue_clone = queue.clone();
            let executor = Arc::new(DeployExecutor::new());
            let db_clone = state.db.clone();

            tokio::spawn(async move {
                deploy::start_worker(queue_clone, executor, db_clone).await;
            });

            info!("Deployment worker started");
        }
    }

    let app = create_router(state.clone());

    let addr = SocketAddr::new(config.server.bind.parse()?, config.server.port);

    let listener = TcpListener::bind(addr).await?;
    info!(address = %addr, mode = ?config.mode, "Server listening");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown::signal())
    .await?;

    info!("Server shutdown complete");
    Ok(())
}

fn create_router(state: Arc<AppState>) -> Router {
    let config = state.config.clone();

    let mut router = Router::new();

    // Common routes
    router = router.merge(routes::common());

    // Mode-specific routes
    match config.mode {
        Mode::Agent => {
            router = router.merge(routes::agent());
        }
        Mode::Home => {
            router = router.merge(routes::home());
        }
    }

    // Apply security middleware (order matters: first applied = last executed)
    router = router
        .layer(axum_mw::from_fn(middleware::request_timing))
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            middleware::rate_limiting,
        ))
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            middleware::jwt_auth,
        ))
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            middleware::network_isolation,
        ));

    // Apply common middleware (compression, cors, tracing)
    router = middleware::apply(router, state.clone());

    router.with_state(state)
}
