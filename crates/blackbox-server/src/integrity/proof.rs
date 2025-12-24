use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityProof {
    pub expected_checksum: u32,
    pub computed_checksum: u32,
    pub checksum_preview: String, // First 64 chars of checksum string
    pub checksum_len: usize,
    pub top_asks: Vec<(Decimal, Decimal)>, // (price, qty)
    pub top_bids: Vec<(Decimal, Decimal)>, // (price, qty)
    pub verify_latency_ms: u64, // Last latency
    pub last_verify_ts: DateTime<Utc>,
    pub last_mismatch_ts: Option<DateTime<Utc>>,
    pub diagnosis: Option<String>, // Reason for mismatch
    #[serde(skip)]
    latency_history: VecDeque<u64>, // Rolling window for statistics
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyStats {
    pub last_ms: u64,
    pub avg_ms: f64,
    pub p95_ms: u64,
}

impl IntegrityProof {
    pub fn latency_stats(&self) -> LatencyStats {
        if self.latency_history.is_empty() {
            return LatencyStats {
                last_ms: self.verify_latency_ms,
                avg_ms: self.verify_latency_ms as f64,
                p95_ms: self.verify_latency_ms,
            };
        }
        
        let mut sorted: Vec<u64> = self.latency_history.iter().copied().collect();
        sorted.sort();
        
        let sum: u64 = sorted.iter().sum();
        let avg = sum as f64 / sorted.len() as f64;
        
        // P95: 95th percentile
        let p95_index = (sorted.len() as f64 * 0.95) as usize;
        let p95 = sorted.get(p95_index.min(sorted.len() - 1)).copied().unwrap_or(0);
        
        LatencyStats {
            last_ms: self.verify_latency_ms,
            avg_ms: avg,
            p95_ms: p95,
        }
    }
    
    pub fn record_latency(&mut self, latency_ms: u64) {
        self.verify_latency_ms = latency_ms;
        self.latency_history.push_back(latency_ms);
        // Keep last 1000 samples for statistics
        while self.latency_history.len() > 1000 {
            self.latency_history.pop_front();
        }
    }
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
            latency_history: VecDeque::with_capacity(1000),
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

