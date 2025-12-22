use serde_json::json;

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
    // Kraken WS v2 uses "symbol" (singular) not "symbols"
    json!({
        "method": "subscribe",
        "params": {
            "channel": "book",
            "symbol": symbols,  // Try "symbol" instead of "symbols"
            "depth": depth,
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

