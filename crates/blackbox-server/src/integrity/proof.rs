use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityProof {
    pub expected_checksum: u32,
    pub computed_checksum: u32,
    pub checksum_preview: String, // First 64 chars of checksum string
    pub checksum_len: usize,
    pub top_asks: Vec<(Decimal, Decimal)>, // (price, qty)
    pub top_bids: Vec<(Decimal, Decimal)>, // (price, qty)
    pub verify_latency_ms: u64,
    pub last_verify_ts: DateTime<Utc>,
    pub last_mismatch_ts: Option<DateTime<Utc>>,
    pub diagnosis: Option<String>, // Reason for mismatch
}

impl IntegrityProof {
    pub fn new() -> Self {
        Self {
            expected_checksum: 0,
            computed_checksum: 0,
            checksum_preview: String::new(),
            checksum_len: 0,
            top_asks: Vec::new(),
            top_bids: Vec::new(),
            verify_latency_ms: 0,
            last_verify_ts: Utc::now(),
            last_mismatch_ts: None,
            diagnosis: None,
        }
    }

    pub fn is_match(&self) -> bool {
        self.expected_checksum == self.computed_checksum
    }
}

impl Default for IntegrityProof {
    fn default() -> Self {
        Self::new()
    }
}

