use blackbox_core::types::*;
use serde_json::Value;

/// Parse a raw WebSocket frame into a normalized message
pub fn parse_frame(frame: &str) -> anyhow::Result<WsFrame> {
    let json: Value = serde_json::from_str(frame)?;
    
    // Check if it's an ACK/response
    if json.get("method").is_some() || json.get("success").is_some() {
        let ack: WsAck = serde_json::from_value(json)?;
        return Ok(WsFrame::Ack(ack));
    }
    
    // Check channel field
    if let Some(channel) = json.get("channel").and_then(|c| c.as_str()) {
        match channel {
            "book" => {
                let msg: BookMessage = serde_json::from_value(json)?;
                Ok(WsFrame::Book(msg))
            }
            "instrument" => {
                let msg: InstrumentMessage = serde_json::from_value(json)?;
                Ok(WsFrame::Instrument(msg))
            }
            "status" => {
                let msg: StatusMessage = serde_json::from_value(json)?;
                Ok(WsFrame::Status(msg))
            }
            "heartbeat" => {
                // Heartbeat might not have type field, handle gracefully
                let msg = if let Ok(m) = serde_json::from_value::<HeartbeatMessage>(json.clone()) {
                    m
                } else {
                    // Fallback to minimal structure
                    HeartbeatMessage {
                        msg_type: None,
                        data: None,
                    }
                };
                Ok(WsFrame::Heartbeat(msg))
            }
            "ping" => {
                let msg: PingMessage = serde_json::from_value(json)?;
                Ok(WsFrame::Ping(msg))
            }
            _ => Err(anyhow::anyhow!("Unknown channel: {}", channel)),
        }
    } else {
        Err(anyhow::anyhow!("Frame missing 'channel' field"))
    }
}

#[derive(Debug, Clone)]
pub enum WsFrame {
    Ack(WsAck),
    Book(BookMessage),
    Instrument(InstrumentMessage),
    Status(StatusMessage),
    Heartbeat(HeartbeatMessage),
    Ping(PingMessage),
}

