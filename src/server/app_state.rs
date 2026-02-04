use crate::config::{Config, Mode};
use crate::deploy::{DeployExecutor, DeployQueue};
use crate::server::middleware::rate_limit::RateLimiter;
use crate::storage::Database;
use std::sync::Arc;

pub struct AppState {
    pub config: Config,
    pub start_time: std::time::Instant,
    pub rate_limiter: RateLimiter,
    /// Database connection (Home mode only)
    pub db: Option<Arc<Database>>,
    /// Deployment queue
    pub deploy_queue: Option<Arc<DeployQueue>>,
    /// Deployment executor for direct operations (shutdown)
    pub deploy_executor: Option<Arc<DeployExecutor>>,
}

impl AppState {
    pub fn new(config: Config) -> Arc<Self> {
        let (deploy_queue, deploy_executor) = if config.modules.deploy.enabled {
            (
                Some(Arc::new(DeployQueue::default())),
                Some(Arc::new(DeployExecutor::new())),
            )
        } else {
            (None, None)
        };

        Arc::new(Self {
            config,
            start_time: std::time::Instant::now(),
            rate_limiter: RateLimiter::default(),
            db: None,
            deploy_queue,
            deploy_executor,
        })
    }

    pub fn with_database(config: Config, db: Arc<Database>) -> Arc<Self> {
        let (deploy_queue, deploy_executor) = if config.modules.deploy.enabled {
            (
                Some(Arc::new(DeployQueue::default())),
                Some(Arc::new(DeployExecutor::new())),
            )
        } else {
            (None, None)
        };

        Arc::new(Self {
            config,
            start_time: std::time::Instant::now(),
            rate_limiter: RateLimiter::default(),
            db: Some(db),
            deploy_queue,
            deploy_executor,
        })
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    #[allow(dead_code)]
    pub fn is_home_mode(&self) -> bool {
        self.config.mode == Mode::Home
    }
}
