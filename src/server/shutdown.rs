use tokio::signal;
use tracing::info;

pub async fn signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    #[cfg(unix)]
    let hangup = async {
        signal::unix::signal(signal::unix::SignalKind::hangup())
            .expect("Failed to install SIGHUP handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let hangup = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, initiating shutdown");
        }
        _ = terminate => {
            info!("Received SIGTERM, initiating shutdown");
        }
        _ = hangup => {
            info!("Received SIGHUP, config reload requested");
            // TODO: Implement hot-reload
        }
    }
}
