use super::migrations;
use super::models::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Mutex;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

fn parse_rfc3339(s: &str) -> OffsetDateTime {
    OffsetDateTime::parse(s, &Rfc3339).unwrap_or_else(|_| OffsetDateTime::now_utc())
}

fn format_rfc3339(dt: OffsetDateTime) -> String {
    dt.format(&Rfc3339).unwrap_or_else(|_| String::new())
}

pub struct Database {
    pub(crate) conn: Mutex<Connection>,
}

impl Database {
    pub fn new(path: &str) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrent access
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             PRAGMA cache_size=-64000;",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn migrate(&self) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        migrations::run_migrations(&conn)
    }

    // =========================================================================
    // Metrics Operations
    // =========================================================================

    #[allow(dead_code)]
    pub fn insert_metric(&self, metric: &MetricRecord) -> rusqlite::Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO metrics_raw (
                agent_name, collected_at, cpu_usage, memory_usage_percent,
                memory_used, memory_total, load_one, load_five, load_fifteen,
                disk_usage_percent, containers_running, containers_total, raw_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                metric.agent_name,
                format_rfc3339(metric.collected_at),
                metric.cpu_usage,
                metric.memory_usage_percent,
                metric.memory_used as i64,
                metric.memory_total as i64,
                metric.load_one,
                metric.load_five,
                metric.load_fifteen,
                metric.disk_usage_percent,
                metric.containers_running,
                metric.containers_total,
                metric.raw_json,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_metrics(&self, query: &MetricsQuery) -> rusqlite::Result<Vec<MetricRecord>> {
        let conn = self.conn.lock().unwrap();

        let mut sql = String::from(
            "SELECT id, agent_name, collected_at, cpu_usage, memory_usage_percent,
                    memory_used, memory_total, load_one, load_five, load_fifteen,
                    disk_usage_percent, containers_running, containers_total, raw_json
             FROM metrics_raw WHERE 1=1",
        );

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(agent) = &query.agent_name {
            sql.push_str(" AND agent_name = ?");
            params_vec.push(Box::new(agent.clone()));
        }
        if let Some(from) = &query.from {
            sql.push_str(" AND collected_at >= ?");
            params_vec.push(Box::new(format_rfc3339(*from)));
        }
        if let Some(to) = &query.to {
            sql.push_str(" AND collected_at <= ?");
            params_vec.push(Box::new(format_rfc3339(*to)));
        }

        sql.push_str(" ORDER BY collected_at DESC");

        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(MetricRecord {
                id: Some(row.get(0)?),
                agent_name: row.get(1)?,
                collected_at: parse_rfc3339(&row.get::<_, String>(2)?),
                cpu_usage: row.get(3)?,
                memory_usage_percent: row.get(4)?,
                memory_used: row.get::<_, i64>(5)? as u64,
                memory_total: row.get::<_, i64>(6)? as u64,
                load_one: row.get(7)?,
                load_five: row.get(8)?,
                load_fifteen: row.get(9)?,
                disk_usage_percent: row.get(10)?,
                containers_running: row.get(11)?,
                containers_total: row.get(12)?,
                raw_json: row.get(13)?,
            })
        })?;

        rows.collect()
    }

    pub fn get_hourly_metrics(
        &self,
        agent_name: &str,
        from: Option<OffsetDateTime>,
        to: Option<OffsetDateTime>,
    ) -> rusqlite::Result<Vec<AggregatedMetric>> {
        let conn = self.conn.lock().unwrap();

        let mut sql = String::from(
            "SELECT id, agent_name, hour_start, cpu_avg, cpu_max,
                    memory_avg, memory_max, load_avg, load_max, samples_count
             FROM metrics_hourly WHERE agent_name = ?",
        );

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        params_vec.push(Box::new(agent_name.to_string()));

        if let Some(from) = from {
            sql.push_str(" AND hour_start >= ?");
            params_vec.push(Box::new(format_rfc3339(from)));
        }
        if let Some(to) = to {
            sql.push_str(" AND hour_start <= ?");
            params_vec.push(Box::new(format_rfc3339(to)));
        }

        sql.push_str(" ORDER BY hour_start DESC");

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(AggregatedMetric {
                id: Some(row.get(0)?),
                agent_name: row.get(1)?,
                period_start: parse_rfc3339(&row.get::<_, String>(2)?),
                cpu_avg: row.get(3)?,
                cpu_max: row.get(4)?,
                memory_avg: row.get(5)?,
                memory_max: row.get(6)?,
                load_avg: row.get(7)?,
                load_max: row.get(8)?,
                samples_count: row.get(9)?,
            })
        })?;

        rows.collect()
    }

    // =========================================================================
    // Deploy History Operations
    // =========================================================================

    #[allow(dead_code)]
    pub fn insert_deploy(&self, deploy: &DeployRecord) -> rusqlite::Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO deploy_history (
                agent_name, deployment_name, deploy_type, status, started_at,
                completed_at, duration_ms, trigger_source, commit_sha, output, error_message
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                deploy.agent_name,
                deploy.deployment_name,
                deploy.deploy_type,
                deploy.status.to_string(),
                format_rfc3339(deploy.started_at),
                deploy.completed_at.map(format_rfc3339),
                deploy.duration_ms,
                deploy.trigger_source,
                deploy.commit_sha,
                deploy.output,
                deploy.error_message,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    #[allow(dead_code)]
    pub fn update_deploy_status(
        &self,
        id: i64,
        status: DeployStatus,
        completed_at: Option<OffsetDateTime>,
        duration_ms: Option<i64>,
        output: Option<&str>,
        error_message: Option<&str>,
    ) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE deploy_history SET
                status = ?1, completed_at = ?2, duration_ms = ?3, output = ?4, error_message = ?5
             WHERE id = ?6",
            params![
                status.to_string(),
                completed_at.map(format_rfc3339),
                duration_ms,
                output,
                error_message,
                id,
            ],
        )?;
        Ok(())
    }

    pub fn get_deploy_history(
        &self,
        agent_name: Option<&str>,
        limit: u32,
    ) -> rusqlite::Result<Vec<DeployRecord>> {
        let conn = self.conn.lock().unwrap();

        let sql = if agent_name.is_some() {
            "SELECT id, agent_name, deployment_name, deploy_type, status, started_at,
                    completed_at, duration_ms, trigger_source, commit_sha, output, error_message
             FROM deploy_history WHERE agent_name = ?1
             ORDER BY started_at DESC LIMIT ?2"
        } else {
            "SELECT id, agent_name, deployment_name, deploy_type, status, started_at,
                    completed_at, duration_ms, trigger_source, commit_sha, output, error_message
             FROM deploy_history ORDER BY started_at DESC LIMIT ?1"
        };

        let mut stmt = conn.prepare(sql)?;

        let rows = if let Some(agent) = agent_name {
            stmt.query_map(params![agent, limit], Self::map_deploy_row)?
        } else {
            stmt.query_map(params![limit], Self::map_deploy_row)?
        };

        rows.collect()
    }

    fn map_deploy_row(row: &rusqlite::Row) -> rusqlite::Result<DeployRecord> {
        Ok(DeployRecord {
            id: Some(row.get(0)?),
            agent_name: row.get(1)?,
            deployment_name: row.get(2)?,
            deploy_type: row.get(3)?,
            status: row
                .get::<_, String>(4)?
                .parse()
                .unwrap_or(DeployStatus::Pending),
            started_at: parse_rfc3339(&row.get::<_, String>(5)?),
            completed_at: row.get::<_, Option<String>>(6)?.map(|s| parse_rfc3339(&s)),
            duration_ms: row.get(7)?,
            trigger_source: row.get(8)?,
            commit_sha: row.get(9)?,
            output: row.get(10)?,
            error_message: row.get(11)?,
        })
    }

    // =========================================================================
    // Suspicious Requests Operations
    // =========================================================================

    #[allow(dead_code)]
    pub fn insert_suspicious_request(&self, req: &SuspiciousRequest) -> rusqlite::Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO suspicious_requests (
                recorded_at, source_ip, method, path, reason, user_agent, headers
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                format_rfc3339(req.recorded_at),
                req.source_ip,
                req.method,
                req.path,
                req.reason,
                req.user_agent,
                req.headers,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_suspicious_requests(&self, limit: u32) -> rusqlite::Result<Vec<SuspiciousRequest>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, recorded_at, source_ip, method, path, reason, user_agent, headers
             FROM suspicious_requests ORDER BY recorded_at DESC LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit], |row| {
            Ok(SuspiciousRequest {
                id: Some(row.get(0)?),
                recorded_at: parse_rfc3339(&row.get::<_, String>(1)?),
                source_ip: row.get(2)?,
                method: row.get(3)?,
                path: row.get(4)?,
                reason: row.get(5)?,
                user_agent: row.get(6)?,
                headers: row.get(7)?,
            })
        })?;

        rows.collect()
    }

    // =========================================================================
    // Agent Status Operations
    // =========================================================================

    #[allow(dead_code)]
    pub fn update_agent_status(&self, status: &AgentStatus) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO agent_status (agent_name, last_seen, status, version, uptime_seconds)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(agent_name) DO UPDATE SET
                last_seen = ?2, status = ?3, version = ?4, uptime_seconds = ?5",
            params![
                status.agent_name,
                format_rfc3339(status.last_seen),
                status.status,
                status.version,
                status.uptime_seconds.map(|u| u as i64),
            ],
        )?;
        Ok(())
    }

    pub fn get_agent_status(&self, agent_name: &str) -> rusqlite::Result<Option<AgentStatus>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT agent_name, last_seen, status, version, uptime_seconds
             FROM agent_status WHERE agent_name = ?1",
            params![agent_name],
            |row| {
                Ok(AgentStatus {
                    agent_name: row.get(0)?,
                    last_seen: parse_rfc3339(&row.get::<_, String>(1)?),
                    status: row.get(2)?,
                    version: row.get(3)?,
                    uptime_seconds: row.get::<_, Option<i64>>(4)?.map(|u| u as u64),
                })
            },
        )
        .optional()
    }

    pub fn get_all_agent_statuses(&self) -> rusqlite::Result<Vec<AgentStatus>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT agent_name, last_seen, status, version, uptime_seconds
             FROM agent_status ORDER BY agent_name",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(AgentStatus {
                agent_name: row.get(0)?,
                last_seen: parse_rfc3339(&row.get::<_, String>(1)?),
                status: row.get(2)?,
                version: row.get(3)?,
                uptime_seconds: row.get::<_, Option<i64>>(4)?.map(|u| u as u64),
            })
        })?;

        rows.collect()
    }

    // =========================================================================
    // Cleanup Operations
    // =========================================================================

    pub fn cleanup_old_metrics(&self, days: u32) -> rusqlite::Result<usize> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM metrics_raw WHERE collected_at < datetime('now', ?1)",
            params![format!("-{} days", days)],
        )
    }

    pub fn cleanup_old_hourly(&self, days: u32) -> rusqlite::Result<usize> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM metrics_hourly WHERE hour_start < datetime('now', ?1)",
            params![format!("-{} days", days)],
        )
    }

    pub fn cleanup_old_daily(&self, days: u32) -> rusqlite::Result<usize> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM metrics_daily WHERE day_start < datetime('now', ?1)",
            params![format!("-{} days", days)],
        )
    }

    #[allow(dead_code)]
    pub fn cleanup_old_suspicious(&self, days: u32) -> rusqlite::Result<usize> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM suspicious_requests WHERE recorded_at < datetime('now', ?1)",
            params![format!("-{} days", days)],
        )
    }
}
