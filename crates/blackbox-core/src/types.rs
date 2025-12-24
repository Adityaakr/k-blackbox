use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsAck {
    pub method: String,
    pub success: Option<bool>,
    pub result: Option<AckResult>,
    #[serde(deserialize_with = "deserialize_optional_timestamp")]
    pub time_in: Option<u64>,
    #[serde(deserialize_with = "deserialize_optional_timestamp")]
    pub time_out: Option<u64>,
    pub req_id: Option<u64>,
    pub error: Option<String>,
}

fn deserialize_optional_timestamp<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let value: serde_json::Value = serde::Deserialize::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(_s) => {
            // Try to parse as ISO timestamp and convert to unix timestamp
            // For now, just return None since we don't need the timestamp
            Ok(None)
        }
        serde_json::Value::Number(n) => {
            n.as_u64().ok_or_else(|| Error::custom("Invalid number"))
                .map(Some)
        }
        serde_json::Value::Null => Ok(None),
        _ => Ok(None),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AckResult {
    pub channel: Option<String>,
    pub req_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "channel", rename_all = "lowercase")]
pub enum WsMessage {
    #[serde(rename = "book")]
    Book(BookMessage),
    #[serde(rename = "instrument")]
    Instrument(InstrumentMessage),
    #[serde(rename = "status")]
    Status(StatusMessage),
    #[serde(rename = "heartbeat")]
    Heartbeat(HeartbeatMessage),
    #[serde(rename = "ping")]
    Ping(PingMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub data: Vec<BookData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookLevelData {
    pub price: serde_json::Value,  // Can be number or string
    pub qty: serde_json::Value,     // Can be number or string
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookData {
    pub symbol: String,
    pub bids: Option<Vec<BookLevelData>>,
    pub asks: Option<Vec<BookLevelData>>,
    pub checksum: Option<u32>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub data: InstrumentData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentData {
    pub pairs: Vec<InstrumentPair>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentPair {
    pub symbol: String,
    #[serde(rename = "price_precision")]
    pub price_precision: u32,
    #[serde(rename = "qty_precision")]
    pub qty_precision: u32,
    #[serde(rename = "price_increment", deserialize_with = "deserialize_decimal_string")]
    pub price_increment: String,
    #[serde(rename = "qty_increment", deserialize_with = "deserialize_decimal_string")]
    pub qty_increment: String,
    pub status: String,
}

fn deserialize_decimal_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let value: serde_json::Value = serde::Deserialize::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(n) => {
            Ok(n.to_string())
        }
        serde_json::Value::String(s) => Ok(s),
        _ => Err(Error::custom("Expected number or string for decimal")),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub data: StatusData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusData {
    pub system: String,
    pub status: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatMessage {
    #[serde(rename = "type")]
    pub msg_type: Option<String>,
    pub data: Option<HeartbeatData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatData {
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub data: Option<serde_json::Value>,
}

// BookLevel struct moved to BookLevelData above for WebSocket message parsing

#[derive(Debug, Clone, Default, Serialize)]
pub struct InstrumentInfo {
    pub symbol: String,
    pub price_precision: u32,
    pub qty_precision: u32,
    pub price_increment: Decimal,
    pub qty_increment: Decimal,
    pub status: String,
}

pub type InstrumentMap = HashMap<String, InstrumentInfo>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedFrame {
    pub ts: DateTime<Utc>,
    pub raw_frame: String,
    pub decoded_event: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayConfig {
    pub mode: ReplayMode,
    pub fault: FaultRule,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReplayMode {
    Realtime,
    Speed(f64),
    AsFast,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FaultType {
    Drop,
    Reorder,
    MutateQty { delta_ticks: i32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FaultRule {
    Every { n: usize, fault: FaultType },
    OnceAt { index: usize, fault: FaultType },
    None,
}

