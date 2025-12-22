use blackbox_core::health::{HealthStatus, SymbolHealth};
use blackbox_core::orderbook::Orderbook;
use blackbox_core::types::InstrumentInfo;
use chrono::Utc;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub orderbooks: Arc<DashMap<String, Orderbook>>,
    pub instruments: Arc<DashMap<String, InstrumentInfo>>,
    pub health: Arc<DashMap<String, SymbolHealth>>,
    pub depths: Arc<DashMap<String, u32>>, // Track depth per symbol
    pub start_time: Instant,
    pub last_frames: Arc<RwLock<Vec<(chrono::DateTime<Utc>, String)>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            orderbooks: Arc::new(DashMap::new()),
            instruments: Arc::new(DashMap::new()),
            health: Arc::new(DashMap::new()),
            depths: Arc::new(DashMap::new()),
            start_time: Instant::now(),
            last_frames: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    pub fn set_depth(&self, symbol: &str, depth: u32) {
        self.depths.insert(symbol.to_string(), depth);
    }
    
    pub fn get_depth(&self, symbol: &str) -> u32 {
        self.depths.get(symbol).map(|e| *e.value()).unwrap_or(100)
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    pub fn overall_health(&self) -> blackbox_core::health::OverallHealth {
        let symbols: Vec<SymbolHealth> = self.health.iter().map(|e| e.value().clone()).collect();
        let worst_status = symbols.iter()
            .map(|s| s.status())
            .min_by_key(|s| match s {
                HealthStatus::Fail => 0,
                HealthStatus::Warn => 1,
                HealthStatus::Ok => 2,
            })
            .unwrap_or(HealthStatus::Ok);
        
        blackbox_core::health::OverallHealth {
            status: worst_status,
            symbols,
            uptime_seconds: self.uptime_seconds(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

