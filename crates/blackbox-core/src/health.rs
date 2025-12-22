use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct SymbolHealth {
    pub symbol: String,
    pub connected: bool,
    pub last_msg_ts: Option<DateTime<Utc>>,
    pub total_msgs: u64,
    pub checksum_ok: u64,
    pub checksum_fail: u64,
    pub last_checksum_mismatch: Option<DateTime<Utc>>,
    pub consecutive_fails: u64,
    pub reconnect_count: u64,
    pub msg_rate_estimate: f64, // messages per second
}

impl SymbolHealth {
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            ..Default::default()
        }
    }

    pub fn checksum_ok_rate(&self) -> f64 {
        let total = self.checksum_ok + self.checksum_fail;
        if total == 0 {
            1.0
        } else {
            self.checksum_ok as f64 / total as f64
        }
    }

    pub fn health_score(&self) -> u8 {
        let mut score = 100u8;
        
        // Deduct for checksum failures
        let fail_rate = 1.0 - self.checksum_ok_rate();
        if fail_rate > 0.01 {
            // More than 1% failure rate
            score = score.saturating_sub((fail_rate * 100.0) as u8);
        }
        
        // Deduct for consecutive failures
        if self.consecutive_fails > 0 {
            score = score.saturating_sub((self.consecutive_fails.min(10) * 5) as u8);
        }
        
        // Deduct if not connected
        if !self.connected {
            score = score.saturating_sub(50);
        }
        
        // Deduct if stale (no messages in last 60s)
        if let Some(last_ts) = self.last_msg_ts {
            let age = Utc::now().signed_duration_since(last_ts);
            if age.num_seconds() > 60 {
                score = score.saturating_sub(30);
            }
        } else {
            score = score.saturating_sub(30);
        }
        
        score
    }

    pub fn status(&self) -> HealthStatus {
        let score = self.health_score();
        if score >= 90 {
            HealthStatus::Ok
        } else if score >= 70 {
            HealthStatus::Warn
        } else {
            HealthStatus::Fail
        }
    }

    pub fn record_checksum_ok(&mut self) {
        self.checksum_ok += 1;
        self.consecutive_fails = 0;
    }

    pub fn record_checksum_fail(&mut self) {
        self.checksum_fail += 1;
        self.consecutive_fails += 1;
        self.last_checksum_mismatch = Some(Utc::now());
    }

    pub fn record_message(&mut self) {
        self.total_msgs += 1;
        self.last_msg_ts = Some(Utc::now());
    }

    pub fn update_msg_rate(&mut self, rate: f64) {
        self.msg_rate_estimate = rate;
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HealthStatus {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverallHealth {
    pub status: HealthStatus,
    pub symbols: Vec<SymbolHealth>,
    pub uptime_seconds: u64,
}

