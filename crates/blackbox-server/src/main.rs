mod http;
mod metrics;
mod state;
mod static_ui;

use anyhow::Context;
use blackbox_core::checksum::verify_checksum;
use blackbox_core::orderbook::Orderbook;
use blackbox_core::recorder::Recorder;
use blackbox_core::replayer::Replayer;
use blackbox_core::types::{ReplayConfig, ReplayMode};
use blackbox_ws::client::{WsClient, WsEvent};
use clap::{Parser, Subcommand};
use http::router;
use metrics::init_metrics;
use state::AppState;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{error, info, warn};
use axum::response::Html;
use axum::routing::get;

#[derive(Parser)]
#[command(name = "blackbox")]
#[command(about = "Kraken WebSocket v2 market data client with orderbook engine and checksum verification")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the blackbox client
    Run {
        /// Symbols to subscribe to (comma-separated)
        #[arg(long, value_delimiter = ',')]
        symbols: Vec<String>,
        /// Orderbook depth
        #[arg(long, default_value = "100")]
        depth: u32,
        /// HTTP server address
        #[arg(long, default_value = "127.0.0.1:8080")]
        http: String,
        /// Ping interval (e.g., "30s")
        #[arg(long, default_value = "30s")]
        ping_interval: String,
        /// Recording file path (optional)
        #[arg(long)]
        record: Option<PathBuf>,
    },
    /// Replay a recording
    Replay {
        /// Input recording file
        #[arg(long)]
        input: PathBuf,
        /// Replay speed multiplier
        #[arg(long, default_value = "1.0")]
        speed: f64,
        /// HTTP server address
        #[arg(long, default_value = "127.0.0.1:8080")]
        http: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            symbols,
            depth,
            http,
            ping_interval,
            record,
        } => {
            run_client(symbols, depth, http, ping_interval, record).await?;
        }
        Commands::Replay { input, speed, http } => {
            replay_recording(input, speed, http).await?;
        }
    }

    Ok(())
}

async fn run_client(
    symbols: Vec<String>,
    depth: u32,
    http_addr: String,
    ping_interval_str: String,
    record_path: Option<PathBuf>,
) -> anyhow::Result<()> {
    info!("Starting Kraken Blackbox");
    info!("Symbols: {:?}, Depth: {}, HTTP: {}", symbols, depth, http_addr);

    // Parse ping interval
    let ping_interval = parse_duration(&ping_interval_str)
        .context("Invalid ping interval format (e.g., '30s', '1m')")?;

    // Initialize metrics
    init_metrics();
    let _metrics_handle = metrics_exporter_prometheus::PrometheusBuilder::new()
        .install()
        .context("Failed to install Prometheus metrics exporter")?;

    // Create shared state
    let state = AppState::new();
    
    // Set depth for all symbols
    for symbol in &symbols {
        state.set_depth(symbol, depth);
    }

    // Create recorder if needed
    let recorder = if let Some(path) = record_path {
        Some(Recorder::new(path)?)
    } else {
        None
    };

    // Create WebSocket event channel
    let (ws_tx, mut ws_rx) = mpsc::unbounded_channel();

    // Spawn WebSocket client
    let client = WsClient::new(symbols.clone(), depth, ping_interval, ws_tx);
    let client_handle = tokio::spawn(async move {
        if let Err(e) = client.run().await {
            error!("WebSocket client error: {}", e);
        }
    });

    // Spawn orderbook processor
    let state_clone = state.clone();
    let mut recorder_mut = recorder;
    let processor_handle = tokio::spawn(async move {
        process_ws_events(&state_clone, &mut ws_rx, recorder_mut.as_mut()).await;
    });

    // Start HTTP server
    let app = router(state.clone())
        .route("/", get(|| async { Html(static_ui::UI_HTML) }));
    
    let server_handle = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(&http_addr).await.unwrap();
        info!("HTTP server listening on http://{}", http_addr);
        axum::serve(listener, app).await.unwrap();
    });

    // Wait for all tasks
    tokio::select! {
        _ = client_handle => {
            warn!("WebSocket client task ended");
        }
        _ = processor_handle => {
            warn!("Processor task ended");
        }
        _ = server_handle => {
            warn!("HTTP server task ended");
        }
    }

    Ok(())
}

async fn process_ws_events(
    state: &AppState,
    ws_rx: &mut mpsc::UnboundedReceiver<WsEvent>,
    mut recorder: Option<&mut Recorder>,
) {
    while let Some(event) = ws_rx.recv().await {
        match event {
            WsEvent::Connected => {
                info!("WebSocket connected");
            }
            WsEvent::Disconnected => {
                warn!("WebSocket disconnected");
            }
            WsEvent::Frame(raw_frame) => {
                // Record frame
                if let Some(ref mut rec) = recorder {
                    let _ = rec.record_frame(&raw_frame, None);
                }
                
                // Store in ring buffer (keep last 1000 frames)
                let mut frames = state.last_frames.write().await;
                frames.push((chrono::Utc::now(), raw_frame.clone()));
                if frames.len() > 1000 {
                    frames.remove(0);
                }
            }
            WsEvent::InstrumentSnapshot(instruments) => {
                info!("Received instrument snapshot with {} pairs", instruments.len());
                for (symbol, info) in instruments {
                    state.instruments.insert(symbol.clone(), info);
                }
            }
            WsEvent::BookSnapshot {
                symbol,
                bids,
                asks,
                checksum,
            } => {
                // Initialize orderbook
                let asks_len = asks.len();
                let bids_len = bids.len();
                let mut book = Orderbook::new();
                book.apply_snapshot(bids, asks);
                let depth = state.get_depth(&symbol) as usize;
                book.truncate(depth);
                
                // Verify checksum if available
                if let Some(expected_checksum) = checksum {
                    if let Some(instrument) = state.instruments.get(&symbol) {
                        let is_valid = verify_checksum(
                            &book,
                            expected_checksum,
                            instrument.price_precision,
                            instrument.qty_precision,
                        );
                        
                        let mut health = state.health.entry(symbol.clone()).or_insert_with(|| {
                            blackbox_core::health::SymbolHealth::new(symbol.clone())
                        });
                        health.connected = true;
                        health.record_message();
                        
                        if is_valid {
                            health.record_checksum_ok();
                            metrics::record_checksum_ok(&symbol);
                        } else {
                            health.record_checksum_fail();
                            metrics::record_checksum_fail(&symbol);
                            warn!("Checksum mismatch for {}: expected {}, computed different", symbol, expected_checksum);
                        }
                    }
                }
                
                state.orderbooks.insert(symbol.clone(), book);
                metrics::update_orderbook_depth(&symbol, asks_len, bids_len);
            }
            WsEvent::BookUpdate {
                symbol,
                bids,
                asks,
                checksum,
                timestamp: _,
            } => {
                if let Some(mut book_entry) = state.orderbooks.get_mut(&symbol) {
                    // Apply updates
                    book_entry.apply_updates(bids.clone(), asks.clone());
                    
                    // Truncate to configured depth
                    let depth = state.get_depth(&symbol) as usize;
                    book_entry.truncate(depth);
                    
                    // Verify checksum if available
                    if let Some(expected_checksum) = checksum {
                        if let Some(instrument) = state.instruments.get(&symbol) {
                            let is_valid = verify_checksum(
                                &book_entry,
                                expected_checksum,
                                instrument.price_precision,
                                instrument.qty_precision,
                            );
                            
                            let mut health = state.health.entry(symbol.clone()).or_insert_with(|| {
                                blackbox_core::health::SymbolHealth::new(symbol.clone())
                            });
                            health.connected = true;
                            health.record_message();
                            
                            if is_valid {
                                health.record_checksum_ok();
                                metrics::record_checksum_ok(&symbol);
                            } else {
                                health.record_checksum_fail();
                                metrics::record_checksum_fail(&symbol);
                                warn!("Checksum mismatch for {}: expected {}", symbol, expected_checksum);
                            }
                        }
                    }
                    
                    let (asks_depth, bids_depth) = book_entry.depth();
                    metrics::update_orderbook_depth(&symbol, asks_depth, bids_depth);
                }
            }
            WsEvent::Error(err) => {
                error!("WebSocket error: {}", err);
            }
            WsEvent::RateLimitExceeded => {
                warn!("Rate limit exceeded, entering cooldown");
                metrics::record_reconnect();
                sleep(Duration::from_secs(60)).await; // Cooldown period
            }
        }
    }
}

async fn replay_recording(
    input: PathBuf,
    speed: f64,
    http_addr: String,
) -> anyhow::Result<()> {
    info!("Replaying recording from {:?} at {}x speed", input, speed);

    let mode = if speed == 1.0 {
        ReplayMode::Realtime
    } else if speed > 0.0 {
        ReplayMode::Speed(speed)
    } else {
        ReplayMode::AsFast
    };

    let config = ReplayConfig { mode };
    let mut replayer = Replayer::new(input.clone(), config)?;
    replayer.start();

    // Create shared state
    let state = AppState::new();

    // Spawn processor for replay
    let processor_handle = tokio::spawn(async move {
        use blackbox_ws::parser::parse_frame;
        
        // Process replayed frames
        while !replayer.is_done() {
            if let Some(frame) = replayer.next_frame() {
                // Parse frame similar to live processing
                match parse_frame(&frame) {
                    Ok(parsed) => {
                        // Process parsed frame (similar to process_ws_events)
                        // For now, just log
                        info!("Replayed frame: {:?}", parsed);
                    }
                    Err(e) => {
                        warn!("Failed to parse replayed frame: {}", e);
                    }
                }
            } else {
                // Need to wait for next frame timing
                sleep(Duration::from_millis(10)).await;
            }
        }
        info!("Replay completed");
    });

    // Start HTTP server
    let app = router(state.clone())
        .route("/", get(|| async { Html(static_ui::UI_HTML) }));
    
    let server_handle = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(&http_addr).await.unwrap();
        info!("HTTP server listening on http://{}", http_addr);
        axum::serve(listener, app).await.unwrap();
    });

    tokio::select! {
        _ = processor_handle => {
            info!("Replay completed");
        }
        _ = server_handle => {}
    }

    Ok(())
}

fn parse_duration(s: &str) -> anyhow::Result<Duration> {
    let s = s.trim();
    if s.ends_with('s') {
        let secs: u64 = s[..s.len() - 1].parse()?;
        Ok(Duration::from_secs(secs))
    } else if s.ends_with('m') {
        let mins: u64 = s[..s.len() - 1].parse()?;
        Ok(Duration::from_secs(mins * 60))
    } else {
        // Try parsing as seconds
        let secs: u64 = s.parse()?;
        Ok(Duration::from_secs(secs))
    }
}

