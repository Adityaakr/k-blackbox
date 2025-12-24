use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct FaultInjector {
    pub enabled: Arc<AtomicBool>,
    pub symbol: Arc<std::sync::RwLock<Option<String>>>,
    pub fault_type: Arc<std::sync::RwLock<FaultType>>,
}

#[derive(Debug, Clone, Copy)]
pub enum FaultType {
    MutateQty,
    DropUpdate,
}

impl FaultInjector {
    pub fn new() -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(false)),
            symbol: Arc::new(std::sync::RwLock::new(None)),
            fault_type: Arc::new(std::sync::RwLock::new(FaultType::MutateQty)),
        }
    }

    pub fn trigger(&self, symbol: String) {
        self.enabled.store(true, Ordering::SeqCst);
        *self.symbol.write().unwrap() = Some(symbol);
    }

    pub fn should_inject(&self, symbol: &str) -> bool {
        if !self.enabled.load(Ordering::SeqCst) {
            return false;
        }
        let target = self.symbol.read().unwrap();
        target.as_ref().map(|s| s == symbol).unwrap_or(false)
    }

    pub fn consume(&self) -> Option<(String, FaultType)> {
        if !self.enabled.load(Ordering::SeqCst) {
            return None;
        }
        self.enabled.store(false, Ordering::SeqCst);
        let symbol = self.symbol.read().unwrap().clone()?;
        let fault_type = *self.fault_type.read().unwrap();
        Some((symbol, fault_type))
    }
}

impl Default for FaultInjector {
    fn default() -> Self {
        Self::new()
    }
}

