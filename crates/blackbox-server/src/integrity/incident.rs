use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentMeta {
    pub id: String,
    pub symbol: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
    pub zip_path: Option<PathBuf>,
    pub frames_path: Option<PathBuf>,
    pub frame_count: usize,
}

impl IncidentMeta {
    pub fn new(id: String, symbol: String, reason: String) -> Self {
        Self {
            id,
            symbol,
            reason,
            created_at: Utc::now(),
            zip_path: None,
            frames_path: None,
            frame_count: 0,
        }
    }
}

