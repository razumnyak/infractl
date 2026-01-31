use rusqlite::Connection;

/// Database schema version
const SCHEMA_VERSION: i32 = 1;

/// Run all pending migrations
pub fn run_migrations(conn: &Connection) -> rusqlite::Result<()> {
    // Create migrations table if not exists
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;

    let current_version: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if current_version < SCHEMA_VERSION {
        for version in (current_version + 1)..=SCHEMA_VERSION {
            apply_migration(conn, version)?;
            conn.execute(
                "INSERT INTO schema_migrations (version) VALUES (?1)",
                [version],
            )?;
        }
    }

    Ok(())
}

fn apply_migration(conn: &Connection, version: i32) -> rusqlite::Result<()> {
    match version {
        1 => migration_v1(conn),
        _ => Ok(()),
    }
}

/// Initial schema
fn migration_v1(conn: &Connection) -> rusqlite::Result<()> {
    // Raw metrics from agents
    conn.execute(
        "CREATE TABLE IF NOT EXISTS metrics_raw (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_name TEXT NOT NULL,
            collected_at TEXT NOT NULL DEFAULT (datetime('now')),
            cpu_usage REAL,
            memory_usage_percent REAL,
            memory_used INTEGER,
            memory_total INTEGER,
            load_one REAL,
            load_five REAL,
            load_fifteen REAL,
            disk_usage_percent REAL,
            containers_running INTEGER,
            containers_total INTEGER,
            raw_json TEXT
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_metrics_raw_agent_time
         ON metrics_raw(agent_name, collected_at)",
        [],
    )?;

    // Hourly aggregated metrics
    conn.execute(
        "CREATE TABLE IF NOT EXISTS metrics_hourly (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_name TEXT NOT NULL,
            hour_start TEXT NOT NULL,
            cpu_avg REAL,
            cpu_max REAL,
            memory_avg REAL,
            memory_max REAL,
            load_avg REAL,
            load_max REAL,
            samples_count INTEGER,
            UNIQUE(agent_name, hour_start)
        )",
        [],
    )?;

    // Daily aggregated metrics
    conn.execute(
        "CREATE TABLE IF NOT EXISTS metrics_daily (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_name TEXT NOT NULL,
            day_start TEXT NOT NULL,
            cpu_avg REAL,
            cpu_max REAL,
            memory_avg REAL,
            memory_max REAL,
            load_avg REAL,
            load_max REAL,
            samples_count INTEGER,
            UNIQUE(agent_name, day_start)
        )",
        [],
    )?;

    // Deployment history
    conn.execute(
        "CREATE TABLE IF NOT EXISTS deploy_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_name TEXT NOT NULL,
            deployment_name TEXT NOT NULL,
            deploy_type TEXT NOT NULL,
            status TEXT NOT NULL,
            started_at TEXT NOT NULL DEFAULT (datetime('now')),
            completed_at TEXT,
            duration_ms INTEGER,
            trigger_source TEXT,
            commit_sha TEXT,
            output TEXT,
            error_message TEXT
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_deploy_history_agent_time
         ON deploy_history(agent_name, started_at)",
        [],
    )?;

    // Suspicious requests log
    conn.execute(
        "CREATE TABLE IF NOT EXISTS suspicious_requests (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            recorded_at TEXT NOT NULL DEFAULT (datetime('now')),
            source_ip TEXT NOT NULL,
            method TEXT,
            path TEXT,
            reason TEXT NOT NULL,
            user_agent TEXT,
            headers TEXT
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_suspicious_requests_time
         ON suspicious_requests(recorded_at)",
        [],
    )?;

    // Agent status tracking
    conn.execute(
        "CREATE TABLE IF NOT EXISTS agent_status (
            agent_name TEXT PRIMARY KEY,
            last_seen TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'unknown',
            version TEXT,
            uptime_seconds INTEGER
        )",
        [],
    )?;

    Ok(())
}
