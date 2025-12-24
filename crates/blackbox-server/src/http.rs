use crate::incident::IncidentManager;
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
    body::Body,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

pub fn router(state: AppState, incident_manager: std::sync::Arc<crate::incident::IncidentManager>) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/book/:symbol/top", get(book_top_handler))
        .route("/book/:symbol", get(book_handler))
        .route("/metrics", get(metrics_handler))
        .route("/export-bug", post(export_bug_handler))
        .with_state((state, incident_manager))
}

async fn health_handler(State((state, _)): State<(AppState, Arc<IncidentManager>)>) -> impl IntoResponse {
    let overall = state.overall_health();
    Json(overall)
}

async fn book_top_handler(
    State((state, _)): State<(AppState, Arc<IncidentManager>)>,
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
    State((state, _)): State<(AppState, Arc<IncidentManager>)>,
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

async fn export_bug_handler(
    State((state, incident_manager)): State<(AppState, Arc<IncidentManager>)>
) -> axum::response::Response {
    use blackbox_core::incident::IncidentReason;
    
    // Create a manual export incident
    let incident = incident_manager
        .record_incident(
            IncidentReason::ManualExport,
            None,
            serde_json::json!({}),
        )
        .await;
    
    // Export bundle for the first symbol or use current state
    let symbol = state.health.iter().next().map(|e| e.key().clone());
    let symbol_str = symbol.as_deref().unwrap_or("unknown");
    
    let config = serde_json::json!({
        "symbols": state.health.iter().map(|e| e.key().clone()).collect::<Vec<_>>(),
        "timestamp": Utc::now().to_rfc3339(),
    });
    
    let overall = state.overall_health();
    let health = serde_json::to_value(&overall).unwrap();
    
    let instrument = state.instruments.get(symbol_str).map(|e| e.value().clone());
    
    let book_top = state.orderbooks.get(symbol_str).map(|book| {
        serde_json::json!({
            "best_bid": book.best_bid().map(|(p, q)| (p.to_string(), q.to_string())),
            "best_ask": book.best_ask().map(|(p, q)| (p.to_string(), q.to_string())),
        })
    });
    
    let frames = state.last_frames.read().await;
    let frames_vec: Vec<_> = frames.iter().cloned().collect();
    
    match incident_manager
        .export_incident_bundle(
            &incident,
            config,
            health,
            instrument.as_ref(),
            book_top,
            &frames_vec,
            incident.timestamp,
        )
        .await
    {
        Ok(path) => {
            // Read the ZIP file and return it
            match std::fs::read(&path) {
                Ok(zip_bytes) => {
                    Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", "application/zip")
                        .header("Content-Disposition", format!("attachment; filename=\"{}.zip\"", incident.id))
                        .body(Body::from(zip_bytes))
                        .unwrap()
                        .into_response()
                }
                Err(e) => {
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                        "error": format!("Failed to read bundle: {}", e)
                    }))).into_response()
                }
            }
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("Failed to export bundle: {}", e)
            }))).into_response()
        }
    }
}

