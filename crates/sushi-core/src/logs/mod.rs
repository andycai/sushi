use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

const MAX_LOGS: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub message: String,
}

pub struct LogService {
    entries: Arc<RwLock<Vec<LogEntry>>>,
}

impl LogService {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::with_capacity(MAX_LOGS))),
        }
    }

    pub async fn append(&self, level: &str, message: &str) {
        let mut entries = self.entries.write().await;
        if entries.len() >= MAX_LOGS {
            entries.remove(0);
        }
        entries.push(LogEntry {
            timestamp: Utc::now(),
            level: level.to_uppercase(),
            message: message.to_string(),
        });
    }

    pub async fn info(&self, message: &str) {
        self.append("INFO", message).await;
    }

    pub async fn warn(&self, message: &str) {
        self.append("WARN", message).await;
    }

    pub async fn error(&self, message: &str) {
        self.append("ERROR", message).await;
    }

    pub async fn debug(&self, message: &str) {
        self.append("DEBUG", message).await;
    }

    pub async fn list(&self, limit: usize) -> Vec<LogEntry> {
        let entries = self.entries.read().await;
        let start = entries.len().saturating_sub(limit);
        entries[start..].to_vec()
    }
}

impl Default for LogService {
    fn default() -> Self {
        Self::new()
    }
}
