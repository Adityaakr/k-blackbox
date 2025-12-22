use metrics::{counter, gauge, histogram};
use std::sync::OnceLock;

static METRICS_INIT: OnceLock<()> = OnceLock::new();

pub fn init_metrics() {
    METRICS_INIT.get_or_init(|| {
        // Metrics will be registered automatically when used
    });
}

pub fn record_checksum_ok(symbol: &str) {
    counter!("checksum_ok_total", "symbol" => symbol.to_string()).increment(1);
}

pub fn record_checksum_fail(symbol: &str) {
    counter!("checksum_fail_total", "symbol" => symbol.to_string()).increment(1);
}

pub fn record_message(symbol: &str) {
    counter!("messages_total", "symbol" => symbol.to_string()).increment(1);
}

pub fn record_reconnect() {
    counter!("reconnects_total").increment(1);
}

pub fn update_orderbook_depth(symbol: &str, asks: usize, bids: usize) {
    gauge!("orderbook_asks_depth", "symbol" => symbol.to_string()).set(asks as f64);
    gauge!("orderbook_bids_depth", "symbol" => symbol.to_string()).set(bids as f64);
}

pub fn record_latency(symbol: &str, latency_ms: f64) {
    histogram!("message_latency_ms", "symbol" => symbol.to_string()).record(latency_ms);
}

