use serde_json::json;
use tracing::warn;

/// Kraken WebSocket v2 supported depth values
const SUPPORTED_DEPTHS: &[u32] = &[10, 25, 100, 500, 1000];

/// Normalize depth to nearest supported value
pub fn normalize_depth(depth: u32) -> u32 {
    if SUPPORTED_DEPTHS.contains(&depth) {
        return depth;
    }
    // Find nearest supported depth (prefer smaller)
    for &supported in SUPPORTED_DEPTHS {
        if supported >= depth {
            warn!("Depth {} not supported by Kraken, using {}", depth, supported);
            return supported;
        }
    }
    // If larger than max, use max
    warn!("Depth {} exceeds max supported ({}), using max", depth, SUPPORTED_DEPTHS.last().unwrap());
    *SUPPORTED_DEPTHS.last().unwrap()
}

/// Build a subscribe message for instrument channel
pub fn subscribe_instrument(snapshot: bool) -> serde_json::Value {
    json!({
        "method": "subscribe",
        "params": {
            "channel": "instrument",
            "snapshot": snapshot
        }
    })
}

/// Build a subscribe message for book channel
pub fn subscribe_book(symbols: &[String], depth: u32, snapshot: bool) -> serde_json::Value {
    // Normalize depth to supported value
    let normalized_depth = normalize_depth(depth);
    
    // Kraken WS v2 uses "symbol" (singular) not "symbols"
    json!({
        "method": "subscribe",
        "params": {
            "channel": "book",
            "symbol": symbols,
            "depth": normalized_depth,
            "snapshot": snapshot
        }
    })
}

/// Build a ping message
pub fn ping() -> serde_json::Value {
    json!({
        "method": "ping"
    })
}

/// Build an unsubscribe message
pub fn unsubscribe(channel: &str, symbols: Option<&[String]>) -> serde_json::Value {
    let mut params = json!({
        "channel": channel
    });
    
    if let Some(syms) = symbols {
        params["symbols"] = json!(syms);
    }
    
    json!({
        "method": "unsubscribe",
        "params": params
    })
}

