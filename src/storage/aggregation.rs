use super::repository::Database;
use std::sync::Arc;
use tracing::{error, info};

/// Run hourly aggregation for all agents
pub fn aggregate_hourly(db: &Database) -> rusqlite::Result<u32> {
    let conn_guard = db.conn.lock().unwrap();

    // Aggregate raw metrics into hourly buckets
    let affected = conn_guard.execute(
        "INSERT OR REPLACE INTO metrics_hourly (
            agent_name, hour_start, cpu_avg, cpu_max, memory_avg, memory_max,
            load_avg, load_max, samples_count
        )
        SELECT
            agent_name,
            strftime('%Y-%m-%dT%H:00:00Z', collected_at) as hour_start,
            AVG(cpu_usage) as cpu_avg,
            MAX(cpu_usage) as cpu_max,
            AVG(memory_usage_percent) as memory_avg,
            MAX(memory_usage_percent) as memory_max,
            AVG(load_one) as load_avg,
            MAX(load_one) as load_max,
            COUNT(*) as samples_count
        FROM metrics_raw
        WHERE collected_at >= datetime('now', '-2 hours')
        GROUP BY agent_name, strftime('%Y-%m-%dT%H:00:00Z', collected_at)",
        [],
    )?;

    Ok(affected as u32)
}

/// Run daily aggregation for all agents
pub fn aggregate_daily(db: &Database) -> rusqlite::Result<u32> {
    let conn_guard = db.conn.lock().unwrap();

    // Aggregate hourly metrics into daily buckets
    let affected = conn_guard.execute(
        "INSERT OR REPLACE INTO metrics_daily (
            agent_name, day_start, cpu_avg, cpu_max, memory_avg, memory_max,
            load_avg, load_max, samples_count
        )
        SELECT
            agent_name,
            strftime('%Y-%m-%dT00:00:00Z', hour_start) as day_start,
            AVG(cpu_avg) as cpu_avg,
            MAX(cpu_max) as cpu_max,
            AVG(memory_avg) as memory_avg,
            MAX(memory_max) as memory_max,
            AVG(load_avg) as load_avg,
            MAX(load_max) as load_max,
            SUM(samples_count) as samples_count
        FROM metrics_hourly
        WHERE hour_start >= datetime('now', '-2 days')
        GROUP BY agent_name, strftime('%Y-%m-%dT00:00:00Z', hour_start)",
        [],
    )?;

    Ok(affected as u32)
}

/// Run retention cleanup based on configuration
pub fn run_retention_cleanup(
    db: &Database,
    raw_days: u32,
    hourly_days: u32,
    daily_days: u32,
) -> rusqlite::Result<(usize, usize, usize)> {
    let raw_deleted = db.cleanup_old_metrics(raw_days)?;
    let hourly_deleted = db.cleanup_old_hourly(hourly_days)?;
    let daily_deleted = db.cleanup_old_daily(daily_days)?;

    info!(
        raw = raw_deleted,
        hourly = hourly_deleted,
        daily = daily_deleted,
        "Retention cleanup completed"
    );

    Ok((raw_deleted, hourly_deleted, daily_deleted))
}

/// Parse retention duration string like "7d", "30d" to days
pub fn parse_retention_days(duration: &str) -> u32 {
    let duration = duration.trim().to_lowercase();

    if duration.ends_with('d') {
        duration[..duration.len() - 1].parse().unwrap_or(7)
    } else if duration.ends_with('w') {
        duration[..duration.len() - 1].parse::<u32>().unwrap_or(1) * 7
    } else if duration.ends_with('m') {
        duration[..duration.len() - 1].parse::<u32>().unwrap_or(1) * 30
    } else if duration.ends_with('y') {
        duration[..duration.len() - 1].parse::<u32>().unwrap_or(1) * 365
    } else {
        duration.parse().unwrap_or(7)
    }
}

/// Background task for periodic aggregation and cleanup
pub async fn aggregation_task(db: Arc<Database>, interval_secs: u64) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));

    loop {
        interval.tick().await;

        // Run hourly aggregation
        match aggregate_hourly(&db) {
            Ok(count) => {
                if count > 0 {
                    info!(records = count, "Hourly aggregation completed");
                }
            }
            Err(e) => error!("Hourly aggregation failed: {}", e),
        }
    }
}

/// Background task for daily aggregation
pub async fn daily_aggregation_task(db: Arc<Database>) {
    // Run at startup and then every 24 hours
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(86400));

    loop {
        interval.tick().await;

        match aggregate_daily(&db) {
            Ok(count) => {
                if count > 0 {
                    info!(records = count, "Daily aggregation completed");
                }
            }
            Err(e) => error!("Daily aggregation failed: {}", e),
        }
    }
}

/// Background task for retention cleanup
pub async fn retention_task(db: Arc<Database>, raw_days: u32, hourly_days: u32, daily_days: u32) {
    // Run every 6 hours
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(6 * 3600));

    loop {
        interval.tick().await;

        if let Err(e) = run_retention_cleanup(&db, raw_days, hourly_days, daily_days) {
            error!("Retention cleanup failed: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_retention_days() {
        assert_eq!(parse_retention_days("7d"), 7);
        assert_eq!(parse_retention_days("30d"), 30);
        assert_eq!(parse_retention_days("1w"), 7);
        assert_eq!(parse_retention_days("4w"), 28);
        assert_eq!(parse_retention_days("1m"), 30);
        assert_eq!(parse_retention_days("1y"), 365);
        assert_eq!(parse_retention_days("365d"), 365);
    }
}
