use crate::integrity::proof::IntegrityProof;
use blackbox_core::checksum::{build_checksum_string, compute_crc32};
use blackbox_core::orderbook::Orderbook;
use chrono::Utc;
use rust_decimal::Decimal;
use std::time::Instant;

pub fn update_integrity_proof(
    proof: &mut IntegrityProof,
    book: &Orderbook,
    expected_checksum: u32,
    price_precision: u32,
    qty_precision: u32,
) -> bool {
    let start = Instant::now();
    
    // Build checksum string
    let checksum_string = build_checksum_string(book, price_precision, qty_precision);
    let computed = compute_crc32(&checksum_string);
    
    let latency_ms = start.elapsed().as_millis() as u64;
    
    // Get top 10 bids and asks
    let top_asks: Vec<(Decimal, Decimal)> = book
        .asks_vec(Some(10))
        .into_iter()
        .map(|(p, q)| (p, q))
        .collect();
    
    let top_bids: Vec<(Decimal, Decimal)> = book
        .bids_vec(Some(10))
        .into_iter()
        .map(|(p, q)| (p, q))
        .collect();
    
    // Update proof
    proof.expected_checksum = expected_checksum;
    proof.computed_checksum = computed;
    proof.checksum_preview = checksum_string.chars().take(64).collect();
    proof.checksum_len = checksum_string.len();
    proof.top_asks = top_asks;
    proof.top_bids = top_bids;
    proof.verify_latency_ms = latency_ms;
    proof.last_verify_ts = Utc::now();
    
    let is_match = expected_checksum == computed;
    
    if !is_match {
        proof.last_mismatch_ts = Some(Utc::now());
        proof.diagnosis = Some(format!("Expected 0x{:08X} but computed 0x{:08X}", expected_checksum, computed));
    }
    
    is_match
}

