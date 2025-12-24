use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IncidentReason {
    ChecksumMismatch,
    RateLimit,
    Disconnect,
    ManualExport,
    FaultInject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incident {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub reason: IncidentReason,
    pub symbol: Option<String>,
    pub metadata: serde_json::Value,
}

impl Incident {
    pub fn new(reason: IncidentReason, symbol: Option<String>) -> Self {
        let id = format!(
            "incident_{}_{}",
            Utc::now().timestamp(),
            reason_str(&reason)
        );
        Self {
            id,
            timestamp: Utc::now(),
            reason,
            symbol,
            metadata: serde_json::json!({}),
        }
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

fn reason_str(reason: &IncidentReason) -> &str {
    match reason {
        IncidentReason::ChecksumMismatch => "checksum",
        IncidentReason::RateLimit => "ratelimit",
        IncidentReason::Disconnect => "disconnect",
        IncidentReason::ManualExport => "manual",
        IncidentReason::FaultInject => "fault",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentMetadata {
    pub incident: Incident,
    pub config: serde_json::Value,
    pub health: serde_json::Value,
    pub instrument: Option<serde_json::Value>,
    pub book_top: Option<serde_json::Value>,
}

