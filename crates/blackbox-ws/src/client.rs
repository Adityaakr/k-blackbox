use crate::parser::{parse_frame, WsFrame};
use crate::subscriptions::{ping, subscribe_book, subscribe_instrument};
use anyhow::Context;
use blackbox_core::types::InstrumentInfo;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

const WS_URL: &str = "wss://ws.kraken.com/v2";
const DEFAULT_PING_INTERVAL: Duration = Duration::from_secs(30);
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(300); // 5 minutes
const INITIAL_RECONNECT_DELAY: Duration = Duration::from_secs(1);

pub struct WsClient {
    symbols: Vec<String>,
    depth: u32,
    ping_interval: Duration,
    tx: mpsc::UnboundedSender<WsEvent>,
}

#[derive(Debug, Clone)]
pub enum WsEvent {
    Connected,
    Disconnected,
    Frame(String),
    InstrumentSnapshot(HashMap<String, InstrumentInfo>),
    BookSnapshot { symbol: String, bids: Vec<(rust_decimal::Decimal, rust_decimal::Decimal)>, asks: Vec<(rust_decimal::Decimal, rust_decimal::Decimal)>, checksum: Option<u32> },
    BookUpdate { symbol: String, bids: Vec<(rust_decimal::Decimal, rust_decimal::Decimal)>, asks: Vec<(rust_decimal::Decimal, rust_decimal::Decimal)>, checksum: Option<u32>, timestamp: Option<String> },
    Error(String),
    RateLimitExceeded,
}

impl WsClient {
    pub fn new(
        symbols: Vec<String>,
        depth: u32,
        ping_interval: Duration,
        tx: mpsc::UnboundedSender<WsEvent>,
    ) -> Self {
        Self {
            symbols,
            depth,
            ping_interval,
            tx,
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let mut reconnect_delay = INITIAL_RECONNECT_DELAY;
        let mut reconnect_count = 0u64;
        
        loop {
            match self.connect_and_run().await {
                Ok(()) => {
                    // Normal disconnect, reset delay
                    reconnect_delay = INITIAL_RECONNECT_DELAY;
                    reconnect_count += 1;
                    let _ = self.tx.send(WsEvent::Disconnected);
                }
                Err(e) => {
                    error!("Connection error: {}", e);
                    reconnect_count += 1;
                    let _ = self.tx.send(WsEvent::Disconnected);
                }
            }
            
            // Exponential backoff with jitter
            let jitter = Duration::from_millis(rand::random::<u64>() % 1000);
            let delay = reconnect_delay + jitter;
            warn!("Reconnecting in {:?} (attempt {})", delay, reconnect_count);
            sleep(delay).await;
            
            reconnect_delay = (reconnect_delay * 2).min(MAX_RECONNECT_DELAY);
        }
    }

    async fn connect_and_run(&self) -> anyhow::Result<()> {
        info!("Connecting to {}", WS_URL);
        let (ws_stream, _) = connect_async(WS_URL)
            .await
            .context("Failed to connect to Kraken WebSocket")?;
        
        let (mut write, mut read) = ws_stream.split();
        let _ = self.tx.send(WsEvent::Connected);
        
        // Channel for ping messages
        let (ping_tx, mut ping_rx) = mpsc::unbounded_channel();
        
        // Subscribe to instrument first
        let instrument_sub = subscribe_instrument(true);
        let msg = serde_json::to_string(&instrument_sub)?;
        write.send(Message::Text(msg)).await?;
        info!("Subscribed to instrument channel");
        
        // Wait for instrument snapshot
        let mut instruments_received = false;
        let mut instruments: HashMap<String, InstrumentInfo> = HashMap::new();
        
        // Spawn ping task
        let ping_interval = self.ping_interval;
        let ping_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(ping_interval);
            loop {
                interval.tick().await;
                let ping_msg = ping();
                if let Ok(msg) = serde_json::to_string(&ping_msg) {
                    if ping_tx.send(msg).is_err() {
                        break;
                    }
                    debug!("Queued ping");
                }
            }
        });
        
        // Main read loop with ping handling
        let mut last_activity = Instant::now();
        
        loop {
            tokio::select! {
                msg_opt = read.next() => {
                    match msg_opt {
                        Some(Ok(msg)) => {
                            last_activity = Instant::now();
                            match msg {
                                Message::Text(text) => {
                                    // Check for rate limit error
                                    if text.contains("Exceeded msg rate") || text.contains("rate limit") {
                                        warn!("Rate limit exceeded, entering cooldown");
                                        let _ = self.tx.send(WsEvent::RateLimitExceeded);
                                        // Close connection and reconnect after delay
                                        drop(ping_task);
                                        return Err(anyhow::anyhow!("Rate limit exceeded"));
                                    }
                                    
                                    let _ = self.tx.send(WsEvent::Frame(text.clone()));
                                    
                                    match parse_frame(&text) {
                                        Ok(frame) => {
                                            match frame {
                                                WsFrame::Instrument(msg) => {
                                                    debug!("Received instrument message, type: {:?}, pairs count: {}", msg.msg_type, msg.data.pairs.len());
                                                    if msg.msg_type == "snapshot" {
                                                        use blackbox_core::precision::parse_decimal;
                                                        for pair in msg.data.pairs {
                                                            match (parse_decimal(&pair.price_increment), parse_decimal(&pair.qty_increment)) {
                                                                (Ok(price_inc), Ok(qty_inc)) => {
                                                                    let info = InstrumentInfo {
                                                                        symbol: pair.symbol.clone(),
                                                                        price_precision: pair.price_precision,
                                                                        qty_precision: pair.qty_precision,
                                                                        price_increment: price_inc,
                                                                        qty_increment: qty_inc,
                                                                        status: pair.status,
                                                                    };
                                                                    instruments.insert(pair.symbol, info);
                                                                }
                                                                (Err(e), _) | (_, Err(e)) => {
                                                                    warn!("Failed to parse increment for {}: {}", pair.symbol, e);
                                                                }
                                                            }
                                                        }
                                                        
                                                        if !instruments_received {
                                                            instruments_received = true;
                                                            info!("Received instrument snapshot with {} pairs", instruments.len());
                                                            let _ = self.tx.send(WsEvent::InstrumentSnapshot(instruments.clone()));
                                                            
                                                            // Now subscribe to book
                                                            let book_sub = subscribe_book(&self.symbols, self.depth, true);
                                                            match serde_json::to_string(&book_sub) {
                                                                Ok(msg) => {
                                                                    debug!("Sending book subscription: {}", msg);
                                                                    if let Err(e) = write.send(Message::Text(msg)).await {
                                                                        error!("Failed to send book subscription: {}", e);
                                                                        return Err(anyhow::anyhow!("Failed to send book subscription: {}", e));
                                                                    }
                                                                    info!("Subscribed to book channel for symbols: {:?}", self.symbols);
                                                                }
                                                                Err(e) => {
                                                                    error!("Failed to serialize book subscription: {}", e);
                                                                    return Err(anyhow::anyhow!("Failed to serialize book subscription: {}", e));
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                WsFrame::Book(msg) => {
                                                    for data in msg.data {
                                                        use blackbox_core::precision::parse_decimal;
                                                        
                                                        let mut bids = Vec::new();
                                                        let mut asks = Vec::new();
                                                        
                                                        if let Some(bid_levels) = data.bids {
                                                            for level in bid_levels {
                                                                let price_str = match &level.price {
                                                                    serde_json::Value::Number(n) => n.to_string(),
                                                                    serde_json::Value::String(s) => s.clone(),
                                                                    _ => continue,
                                                                };
                                                                let qty_str = match &level.qty {
                                                                    serde_json::Value::Number(n) => n.to_string(),
                                                                    serde_json::Value::String(s) => s.clone(),
                                                                    _ => continue,
                                                                };
                                                                match (parse_decimal(&price_str), parse_decimal(&qty_str)) {
                                                                    (Ok(price), Ok(qty)) => bids.push((price, qty)),
                                                                    _ => continue,
                                                                }
                                                            }
                                                        }
                                                        
                                                        if let Some(ask_levels) = data.asks {
                                                            for level in ask_levels {
                                                                let price_str = match &level.price {
                                                                    serde_json::Value::Number(n) => n.to_string(),
                                                                    serde_json::Value::String(s) => s.clone(),
                                                                    _ => continue,
                                                                };
                                                                let qty_str = match &level.qty {
                                                                    serde_json::Value::Number(n) => n.to_string(),
                                                                    serde_json::Value::String(s) => s.clone(),
                                                                    _ => continue,
                                                                };
                                                                match (parse_decimal(&price_str), parse_decimal(&qty_str)) {
                                                                    (Ok(price), Ok(qty)) => asks.push((price, qty)),
                                                                    _ => continue,
                                                                }
                                                            }
                                                        }
                                                        
                                                        if msg.msg_type == "snapshot" {
                                                            let _ = self.tx.send(WsEvent::BookSnapshot {
                                                                symbol: data.symbol,
                                                                bids,
                                                                asks,
                                                                checksum: data.checksum,
                                                            });
                                                        } else {
                                                            let _ = self.tx.send(WsEvent::BookUpdate {
                                                                symbol: data.symbol,
                                                                bids,
                                                                asks,
                                                                checksum: data.checksum,
                                                                timestamp: data.timestamp,
                                                            });
                                                        }
                                                    }
                                                }
                                                WsFrame::Heartbeat(_) => {
                                                    debug!("Received heartbeat");
                                                }
                                                WsFrame::Ping(_) => {
                                                    debug!("Received ping");
                                                }
                                                WsFrame::Status(msg) => {
                                                    info!("Status: {} - {}", msg.data.system, msg.data.status);
                                                }
                                                WsFrame::Ack(ack) => {
                                                    if let Some(err) = &ack.error {
                                                        error!("ACK error: {}", err);
                                                        let _ = self.tx.send(WsEvent::Error(err.clone()));
                                                    } else {
                                                        debug!("ACK: method={}, success={:?}", ack.method, ack.success);
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            warn!("Failed to parse frame: {} (frame: {})", e, text);
                                        }
                                    }
                                }
                                Message::Close(_) => {
                                    info!("WebSocket closed by server");
                                    break;
                                }
                                Message::Ping(_) | Message::Pong(_) => {
                                    // Handle automatically by tokio-tungstenite
                                }
                                _ => {}
                            }
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            break;
                        }
                        None => {
                            info!("WebSocket stream ended");
                            break;
                        }
                    }
                }
                ping_msg_opt = ping_rx.recv() => {
                    if let Some(ping_msg) = ping_msg_opt {
                        if write.send(Message::Text(ping_msg)).await.is_err() {
                            break;
                        }
                        debug!("Sent ping");
                    } else {
                        // Ping channel closed
                        break;
                    }
                }
            }
            
            // Check for idle timeout
            if last_activity.elapsed() > IDLE_TIMEOUT {
                warn!("Idle timeout, reconnecting");
                break;
            }
        }
        
        drop(ping_task);
        Ok(())
    }
}

// Add a simple random function since we don't want to add rand dependency just for jitter
mod rand {
    use std::sync::atomic::{AtomicU64, Ordering};
    
    static SEED: AtomicU64 = AtomicU64::new(12345);
    
    pub fn random<T>() -> T
    where
        T: From<u64>,
    {
        let mut seed = SEED.load(Ordering::Relaxed);
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        SEED.store(seed, Ordering::Relaxed);
        T::from(seed)
    }
}

