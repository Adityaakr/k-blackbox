use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use blackbox_core::types::InstrumentInfo;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use zip::ZipWriter;
use zip::write::FileOptions;
use std::io::Write;

#[derive(Deserialize)]
struct BookQuery {
    limit: Option<usize>,
}

#[derive(Serialize)]
struct TopOfBook {
    symbol: String,
    best_bid: Option<(String, String)>,
    best_ask: Option<(String, String)>,
    spread: Option<String>,
    mid: Option<String>,
}

#[derive(Serialize)]
struct BookResponse {
    symbol: String,
    bids: Vec<(String, String)>,
    asks: Vec<(String, String)>,
}

#[derive(Serialize)]
struct ExportBugResponse {
    path: String,
    incident_id: String,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/book/:symbol/top", get(book_top_handler))
        .route("/book/:symbol", get(book_handler))
        .route("/metrics", get(metrics_handler))
        .route("/export-bug", post(export_bug_handler))
        .with_state(state)
}

async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    let overall = state.overall_health();
    Json(overall)
}

async fn book_top_handler(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> impl IntoResponse {
    if let Some(book) = state.orderbooks.get(&symbol) {
        let best_bid = book.best_bid().map(|(p, q)| (p.to_string(), q.to_string()));
        let best_ask = book.best_ask().map(|(p, q)| (p.to_string(), q.to_string()));
        let spread = book.spread().map(|s| s.to_string());
        let mid = book.mid().map(|m| m.to_string());
        
        Json(TopOfBook {
            symbol,
            best_bid,
            best_ask,
            spread,
            mid,
        }).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(TopOfBook {
            symbol,
            best_bid: None,
            best_ask: None,
            spread: None,
            mid: None,
        })).into_response()
    }
}

async fn book_handler(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(params): Query<BookQuery>,
) -> impl IntoResponse {
    if let Some(book) = state.orderbooks.get(&symbol) {
        let limit = params.limit;
        let bids: Vec<(String, String)> = book.bids_vec(limit)
            .iter()
            .map(|(p, q)| (p.to_string(), q.to_string()))
            .collect();
        let asks: Vec<(String, String)> = book.asks_vec(limit)
            .iter()
            .map(|(p, q)| (p.to_string(), q.to_string()))
            .collect();
        
        Json(BookResponse {
            symbol,
            bids,
            asks,
        }).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(BookResponse {
            symbol,
            bids: vec![],
            asks: vec![],
        })).into_response()
    }
}

async fn metrics_handler() -> impl IntoResponse {
    // For now, return a simple metrics endpoint
    // In production, you'd want to set up Prometheus exporter properly
    (StatusCode::OK, "# Prometheus metrics endpoint\n# Install metrics exporter in main.rs\n")
}

async fn export_bug_handler(State(state): State<AppState>) -> axum::response::Response {
    let incident_id = format!("incident_{}", Utc::now().timestamp());
    let output_path = format!("./bug_bundles/{}.zip", incident_id);
    
    // Create directory if needed
    if let Some(parent) = PathBuf::from(&output_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    
    // Create zip file
    let file = match std::fs::File::create(&output_path) {
        Ok(f) => f,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("Failed to create bug bundle: {}", e)
            }))).into_response();
        }
    };
    
    let mut zip = ZipWriter::new(std::io::BufWriter::new(file));
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    
    // Write config.json
    let config = serde_json::json!({
        "symbols": state.health.iter().map(|e| e.key().clone()).collect::<Vec<_>>(),
        "timestamp": Utc::now().to_rfc3339(),
    });
    zip.start_file("config.json", options).unwrap();
    zip.write_all(serde_json::to_string_pretty(&config).unwrap().as_bytes()).unwrap();
    
    // Write health.json
    let overall = state.overall_health();
    zip.start_file("health.json", options).unwrap();
    zip.write_all(serde_json::to_string_pretty(&overall).unwrap().as_bytes()).unwrap();
    
    // Write frames.ndjson (last 60 seconds)
    let frames = state.last_frames.read().await;
    let cutoff = Utc::now() - chrono::Duration::seconds(60);
    let recent_frames: Vec<_> = frames.iter()
        .filter(|(ts, _)| *ts >= cutoff)
        .collect();
    
    zip.start_file("frames.ndjson", options).unwrap();
    for (ts, frame) in recent_frames {
        let line = format!("{{\"ts\":\"{}\",\"raw_frame\":{}}}\n", ts.to_rfc3339(), frame);
        zip.write_all(line.as_bytes()).unwrap();
    }
    
    // Write instrument snapshot
    let instruments: HashMap<String, InstrumentInfo> = state.instruments.iter()
        .map(|e| (e.key().clone(), e.value().clone()))
        .collect();
    zip.start_file("instruments.json", options).unwrap();
    zip.write_all(serde_json::to_string_pretty(&instruments).unwrap().as_bytes()).unwrap();
    
    zip.finish().unwrap();
    
    Json(ExportBugResponse {
        path: output_path,
        incident_id,
    }).into_response()
}

