pub mod aggregation;
mod migrations;
mod models;
mod repository;

pub use aggregation::parse_retention_days;
pub use models::*;
pub use repository::Database;

use crate::config::Config;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use tracing::info;

/// Initialize the database for Home mode
pub async fn init(config: &Config) -> Result<Arc<Database>> {
    let db_path = &config.modules.storage.db_path;

    // Ensure parent directory exists
    if let Some(parent) = Path::new(db_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    info!(path = %db_path, "Initializing database");

    let db = Database::new(db_path)?;
    db.migrate()?;

    Ok(Arc::new(db))
}
