//! Lightweight history store: SQLite with a 10GB cap. Used for dashboard "History" tab.

use crate::model::HistoryOpportunity;
use axum::{Router, extract::State, routing::get, Json};
use log::{info, warn};
use rusqlite::Connection;
use serde::Serialize;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use tower_http::cors::{CorsLayer, Any};

const MAX_DB_BYTES: i64 = 10 * 1024 * 1024 * 1024; // 10 GB
const TARGET_AFTER_CLEANUP_BYTES: i64 = 9 * 1024 * 1024 * 1024; // 9 GB

pub struct HistoryStore {
    conn: Mutex<Connection>,
}

impl HistoryStore {
    pub fn new(path: &Path) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )))
            })?;
        }
        let conn = Connection::open(path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS snapshots (ts INTEGER PRIMARY KEY, data TEXT NOT NULL)",
            [],
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Append one snapshot. Cleans up oldest rows if total size would exceed 10GB.
    pub fn add_snapshot(&self, opportunities: Vec<HistoryOpportunity>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if opportunities.is_empty() {
            return Ok(());
        }
        let ts = chrono::Utc::now().timestamp_millis();
        let data = serde_json::to_string(&opportunities)?;
        let len = data.len() as i64;

        let conn = self.conn.lock().map_err(|e| format!("lock: {}", e))?;
        conn.execute("INSERT INTO snapshots (ts, data) VALUES (?1, ?2)", rusqlite::params![ts, data])?;
        drop(conn);

        self.cleanup_if_needed(len)?;
        Ok(())
    }

    fn total_size(conn: &Connection) -> Result<i64, rusqlite::Error> {
        let mut stmt = conn.prepare("SELECT COALESCE(SUM(LENGTH(data)), 0) FROM snapshots")?;
        let mut rows = stmt.query([])?;
        let size: i64 = if let Some(row) = rows.next()? {
            row.get(0)?
        } else {
            0
        };
        Ok(size)
    }

    fn cleanup_if_needed(&self, _just_added_len: i64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {}", e))?;
        let total = Self::total_size(&conn)?;
        drop(conn);

        if total <= MAX_DB_BYTES {
            return Ok(());
        }

        info!("History DB size {} MB, trimming to under {} GB", total / (1024 * 1024), TARGET_AFTER_CLEANUP_BYTES / (1024 * 1024 * 1024));
        self.trim_oldest_until_under(TARGET_AFTER_CLEANUP_BYTES)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    fn trim_oldest_until_under(&self, target_bytes: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        loop {
            let total: i64 = conn.query_row("SELECT COALESCE(SUM(LENGTH(data)), 0) FROM snapshots", [], |r| r.get(0))?;
            if total <= target_bytes {
                break;
            }
            // Delete oldest (smallest ts) row
            let deleted = conn.execute(
                "DELETE FROM snapshots WHERE ts = (SELECT MIN(ts) FROM snapshots)",
                [],
            )?;
            if deleted == 0 {
                warn!("History cleanup: no rows to delete but size still {} > {}", total, target_bytes);
                break;
            }
        }
        Ok(())
    }

    /// Get all snapshots with timestamp >= since_ts, ordered by ts descending.
    pub fn get_snapshots_since(&self, since_ts: i64) -> Result<Vec<(i64, Vec<HistoryOpportunity>)>, Box<dyn std::error::Error + Send + Sync>> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {}", e))?;
        let mut stmt = conn.prepare("SELECT ts, data FROM snapshots WHERE ts >= ?1 ORDER BY ts DESC")?;
        let rows = stmt.query_map([since_ts], |row| {
            let ts: i64 = row.get(0)?;
            let data: String = row.get(1)?;
            let opportunities: Vec<HistoryOpportunity> = serde_json::from_str(&data).unwrap_or_default();
            Ok((ts, opportunities))
        })?;
        let out: Result<Vec<_>, _> = rows.collect();
        Ok(out?)
    }
}

#[derive(Serialize)]
pub struct SnapshotResponse {
    pub timestamp: i64,
    pub opportunities: Vec<HistoryOpportunity>,
}

async fn history_handler(
    State(store): State<Arc<HistoryStore>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Json<Vec<SnapshotResponse>> {
    let days = params.get("days").and_then(|s| s.parse::<i64>().ok()).unwrap_or(30);
    let since_ts = chrono::Utc::now().timestamp_millis() - days * 24 * 60 * 60 * 1000;
    match store.get_snapshots_since(since_ts) {
        Ok(snapshots) => Json(
            snapshots
                .into_iter()
                .map(|(ts, opportunities)| SnapshotResponse { timestamp: ts, opportunities })
                .collect(),
        ),
        Err(_) => Json(Vec::new()),
    }
}

async fn health_handler() -> &'static str {
    "ok"
}

pub fn router(store: Arc<HistoryStore>) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/api/history", get(history_handler))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(store)
}

pub fn health_only_router() -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .with_state(())
}
