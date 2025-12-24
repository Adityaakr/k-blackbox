use blackbox_core::health::{HealthStatus, SymbolHealth};
use blackbox_core::orderbook::Orderbook;
use blackbox_core::types::InstrumentInfo;
use chrono::Utc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Instant;
use crate::integrity::{IntegrityProof, IncidentMeta};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UiEvent {
    Connected,
    Disconnected,
    SubscribedInstrument,
    SubscribedBook,
    ChecksumOk { symbol: String },
    ChecksumMismatch { symbol: String },
    ResyncStarted { symbol: String },
    ResyncDone { symbol: String },
    RecordStarted { path: String },
    RecordStopped,
    IncidentCaptured { id: String, reason: String },
    IncidentExported { path: String },
    FaultInjected { fault_type: String, symbol: String },
    Error(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct UiEventLogEntry {
    pub timestamp: chrono::DateTime<Utc>,
    pub event: UiEvent,
}

#[derive(Debug, Clone)]
pub struct AggregatedEvent {
    pub timestamp: chrono::DateTime<Utc>,
    pub text: String,
    pub color: crate::tui::widgets::EventColor,
}

#[derive(Clone)]
pub struct AppState {
    pub orderbooks: Arc<DashMap<String, Orderbook>>,
    pub instruments: Arc<DashMap<String, InstrumentInfo>>,
    pub health: Arc<DashMap<String, SymbolHealth>>,
    pub depths: Arc<DashMap<String, u32>>, // Track depth per symbol
    pub start_time: Instant,
    pub last_frames: Arc<RwLock<Vec<(chrono::DateTime<Utc>, String)>>>, // Global frame buffer
    pub per_symbol_frames: Arc<DashMap<String, Arc<RwLock<VecDeque<String>>>>>, // Per-symbol ring buffer
    pub event_log: Arc<RwLock<VecDeque<UiEventLogEntry>>>, // Ring buffer for events
    pub last_incident: Arc<RwLock<Option<IncidentMeta>>>,
    pub incident_count: Arc<RwLock<u64>>,
    pub integrity_proofs: Arc<DashMap<String, IntegrityProof>>, // Per-symbol integrity proofs
    pub fault_injector: Arc<crate::integrity::fault::FaultInjector>, // Fault injection state
    pub requested_symbols: Arc<RwLock<Vec<String>>>, // Symbols requested via CLI args
    pub recording_enabled: Arc<RwLock<bool>>, // Recording toggle state
    pub recording_path: Arc<RwLock<Option<String>>>, // Current recording file path
    pub recorder: Arc<RwLock<Option<blackbox_core::recorder::Recorder>>>, // Shared recorder instance
    pub last_resync: Arc<DashMap<String, Instant>>, // Last resync time per symbol (for backoff)
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
            per_symbol_frames: Arc::new(DashMap::new()),
            event_log: Arc::new(RwLock::new(VecDeque::new())),
            last_incident: Arc::new(RwLock::new(None)),
            incident_count: Arc::new(RwLock::new(0)),
            integrity_proofs: Arc::new(DashMap::new()),
            fault_injector: Arc::new(crate::integrity::fault::FaultInjector::new()),
            requested_symbols: Arc::new(RwLock::new(Vec::new())),
            recording_enabled: Arc::new(RwLock::new(false)),
            recording_path: Arc::new(RwLock::new(None)),
            recorder: Arc::new(RwLock::new(None)),
            last_resync: Arc::new(DashMap::new()),
        }
    }
    
    pub async fn set_recording_enabled(&self, enabled: bool) {
        *self.recording_enabled.write().await = enabled;
    }
    
    pub async fn is_recording_enabled(&self) -> bool {
        *self.recording_enabled.read().await
    }
    
    pub async fn set_recording_path(&self, path: Option<String>) {
        *self.recording_path.write().await = path;
    }
    
    pub async fn get_recording_path(&self) -> Option<String> {
        self.recording_path.read().await.clone()
    }
    
    pub fn can_resync(&self, symbol: &str) -> bool {
        if let Some(last) = self.last_resync.get(symbol) {
            last.elapsed().as_secs() >= 3 // Min 3s between resyncs
        } else {
            true
        }
    }
    
    pub fn record_resync(&self, symbol: &str) {
        self.last_resync.insert(symbol.to_string(), Instant::now());
    }
    
    pub async fn set_requested_symbols(&self, symbols: Vec<String>) {
        *self.requested_symbols.write().await = symbols;
    }
    
    pub async fn get_requested_symbols(&self) -> Vec<String> {
        self.requested_symbols.read().await.clone()
    }
    
    pub fn get_or_create_frame_buffer(&self, symbol: &str) -> Arc<RwLock<VecDeque<String>>> {
        self.per_symbol_frames
            .entry(symbol.to_string())
            .or_insert_with(|| Arc::new(RwLock::new(VecDeque::with_capacity(2000))))
            .value()
            .clone()
    }
    
    pub async fn push_event(&self, event: UiEvent) {
        let mut log = self.event_log.write().await;
        log.push_back(UiEventLogEntry {
            timestamp: Utc::now(),
            event,
        });
        // Keep last 500 events
        while log.len() > 500 {
            log.pop_front();
        }
    }
    
    pub async fn get_events(&self, limit: usize) -> Vec<UiEventLogEntry> {
        let log = self.event_log.read().await;
        let start = log.len().saturating_sub(limit);
        log.iter().skip(start).cloned().collect()
    }
    
    pub async fn get_aggregated_events(&self, limit: usize) -> Vec<AggregatedEvent> {
        let events = self.get_events(1000).await; // Get more to aggregate
        let mut aggregated = Vec::new();
        let mut i = 0;
        
        while i < events.len() && aggregated.len() < limit {
            let current = &events[i];
            match &current.event {
                UiEvent::ChecksumOk { symbol } => {
                    // Count consecutive ChecksumOk
                    let mut count = 1;
                    let mut j = i + 1;
                    while j < events.len() {
                        if let UiEvent::ChecksumOk { symbol: s } = &events[j].event {
                            if s == symbol {
                                count += 1;
                                j += 1;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    if count > 1 {
                        aggregated.push(AggregatedEvent {
                            timestamp: current.timestamp,
                            text: format!("CHECKSUM_OK {} x{}", symbol, count),
                            color: crate::tui::widgets::EventColor::Normal,
                        });
                        i = j;
                    } else {
                        aggregated.push(AggregatedEvent {
                            timestamp: current.timestamp,
                            text: format!("CHECKSUM_OK {}", symbol),
                            color: crate::tui::widgets::EventColor::Normal,
                        });
                        i += 1;
                    }
                }
                UiEvent::ChecksumMismatch { symbol } => {
                    aggregated.push(AggregatedEvent {
                        timestamp: current.timestamp,
                        text: format!("CHECKSUM_MISMATCH {}", symbol),
                        color: crate::tui::widgets::EventColor::Error,
                    });
                    i += 1;
                }
                UiEvent::IncidentExported { path } => {
                    aggregated.push(AggregatedEvent {
                        timestamp: current.timestamp,
                        text: format!("INCIDENT_EXPORTED {}", path),
                        color: crate::tui::widgets::EventColor::Info,
                    });
                    i += 1;
                }
                UiEvent::IncidentCaptured { id, reason } => {
                    aggregated.push(AggregatedEvent {
                        timestamp: current.timestamp,
                        text: format!("INCIDENT_CAPTURED {} ({})", id, reason),
                        color: crate::tui::widgets::EventColor::Error,
                    });
                    i += 1;
                }
                UiEvent::FaultInjected { fault_type, symbol } => {
                    aggregated.push(AggregatedEvent {
                        timestamp: current.timestamp,
                        text: format!("FAULT_INJECTED {} {}", fault_type, symbol),
                        color: crate::tui::widgets::EventColor::Warning,
                    });
                    i += 1;
                }
                _ => {
                    aggregated.push(AggregatedEvent {
                        timestamp: current.timestamp,
                        text: format!("{:?}", current.event),
                        color: crate::tui::widgets::EventColor::Normal,
                    });
                    i += 1;
                }
            }
        }
        
        aggregated
    }
    
    pub async fn set_last_incident(&self, incident: IncidentMeta) {
        let mut count = self.incident_count.write().await;
        *count += 1;
        drop(count);
        
        let mut last = self.last_incident.write().await;
        *last = Some(incident);
    }
    
    pub async fn get_last_incident(&self) -> Option<IncidentMeta> {
        let last = self.last_incident.read().await;
        last.clone()
    }
    
    pub async fn get_incident_count(&self) -> u64 {
        let count = self.incident_count.read().await;
        *count
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

