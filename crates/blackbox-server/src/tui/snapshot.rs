use crate::integrity::IntegrityProof;
use crate::state::AppState;
use blackbox_core::health::HealthStatus;
use chrono::Utc;

#[derive(Clone)]
pub struct UiSnapshot {
    pub mode: String,
    pub connected: bool,
    pub symbols: Vec<String>,
    pub msg_rate: f64,
    pub recording_path: Option<String>,
    pub fault_status: String,
    pub uptime_seconds: u64,
    pub health_status: HealthStatus,
    pub symbol_health: Vec<SymbolHealthRow>,
    pub last_incident: Option<LastIncidentInfo>,
    pub incident_count: u64,
    pub events: Vec<crate::state::AggregatedEvent>,
    pub integrity_proof: Option<IntegrityProof>, // For selected symbol
    pub selected_symbol: Option<String>, // Currently selected symbol
}

#[derive(Clone)]
pub struct SymbolHealthRow {
    pub symbol: String,
    pub checksum_ok: u64,
    pub checksum_fail: u64,
    pub ok_rate: f64,
    pub consecutive_fail: u64,
    pub last_mismatch: Option<String>,
    pub resync_count: u64,
    pub last_msg_age: Option<u64>,
}

#[derive(Clone)]
pub struct LastIncidentInfo {
    pub id: String,
    pub symbol: Option<String>,
    pub reason: String,
    pub timestamp: chrono::DateTime<Utc>,
}

impl UiSnapshot {
    pub async fn from_state(
        state: &AppState,
        mode: &str,
        recording_path: Option<String>,
        fault_status: &str,
        selected_symbol: Option<&str>,
        requested_symbols: Option<&[String]>,
    ) -> Self {
        // Get all symbols from health, but filter to requested ones if provided
        let all_symbols: Vec<String> = state.health.iter().map(|e| e.key().clone()).collect();
        let symbols = if let Some(requested) = requested_symbols {
            // Only show requested symbols, in the order they were requested
            requested.iter()
                .filter(|s| all_symbols.contains(s))
                .cloned()
                .collect()
        } else {
            all_symbols
        };
        
        let mut msg_rate = 0.0;
        for health_entry in state.health.iter() {
            msg_rate += health_entry.value().msg_rate_estimate;
        }
        
        let symbol_health: Vec<SymbolHealthRow> = state
            .health
            .iter()
            .map(|e| {
                let h = e.value();
                let last_mismatch = h.last_checksum_mismatch.map(|ts| {
                    let age = Utc::now().signed_duration_since(ts);
                    if age.num_seconds() < 60 {
                        format!("{}s ago", age.num_seconds())
                    } else if age.num_minutes() < 60 {
                        format!("{}m ago", age.num_minutes())
                    } else {
                        format!("{}h ago", age.num_hours())
                    }
                });
                
                let last_msg_age = h.last_msg_ts.map(|ts| {
                    Utc::now().signed_duration_since(ts).num_seconds() as u64
                });
                
                SymbolHealthRow {
                    symbol: h.symbol.clone(),
                    checksum_ok: h.checksum_ok,
                    checksum_fail: h.checksum_fail,
                    ok_rate: h.checksum_ok_rate(),
                    consecutive_fail: h.consecutive_fails,
                    last_mismatch,
                    resync_count: h.reconnect_count,
                    last_msg_age,
                }
            })
            .collect();
        
        let overall = state.overall_health();
        let connected = overall.symbols.iter().any(|s| s.connected);
        
        let last_incident = state.get_last_incident().await.map(|inc| {
            LastIncidentInfo {
                id: inc.id,
                symbol: Some(inc.symbol),
                reason: inc.reason,
                timestamp: inc.created_at,
            }
        });
        
        let incident_count = state.get_incident_count().await;
        let events = state.get_aggregated_events(30).await;
        
        let integrity_proof = selected_symbol.and_then(|sym| {
            state.integrity_proofs.get(sym).map(|p| p.value().clone())
        });
        
        Self {
            mode: mode.to_string(),
            connected,
            symbols,
            msg_rate,
            recording_path,
            fault_status: fault_status.to_string(),
            uptime_seconds: state.uptime_seconds(),
            health_status: overall.status,
            symbol_health,
            last_incident,
            incident_count,
            events,
            integrity_proof,
            selected_symbol: selected_symbol.map(|s| s.to_string()),
        }
    }
    
    pub fn integrity_badge_status(&self) -> (IntegrityStatus, &'static str) {
        if !self.connected {
            return (IntegrityStatus::Broken, "❌ BROKEN");
        }
        
        if self.symbol_health.is_empty() {
            return (IntegrityStatus::Degraded, "⚠ DEGRADED");
        }
        
        // Check if any symbol has issues
        let has_issues = self.symbol_health.iter().any(|s| {
            s.ok_rate < 0.9999 || s.consecutive_fail > 0
        });
        
        let has_broken = self.symbol_health.iter().any(|s| {
            s.consecutive_fail >= 3
        });
        
        if has_broken {
            (IntegrityStatus::Broken, "❌ BROKEN")
        } else if has_issues {
            (IntegrityStatus::Degraded, "⚠ DEGRADED")
        } else {
            (IntegrityStatus::Verified, "✅ VERIFIED")
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum IntegrityStatus {
    Verified,
    Degraded,
    Broken,
}

