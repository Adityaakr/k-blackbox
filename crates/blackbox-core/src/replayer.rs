use crate::types::{FaultRule, FaultType, RecordedFrame, ReplayConfig, ReplayMode};
use chrono::{DateTime, Utc};
use serde_json;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::Instant;
use tracing::warn;

pub struct Replayer {
    frames: Vec<(DateTime<Utc>, String)>,
    current_index: usize,
    start_time: Option<Instant>,
    first_frame_time: Option<DateTime<Utc>>,
    config: ReplayConfig,
    book_update_count: HashMap<String, usize>,
    fault_applied: bool,
    next_frame_buffer: Option<String>,
}

impl Replayer {
    pub fn new(path: PathBuf, config: ReplayConfig) -> anyhow::Result<Self> {
        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        
        let mut frames = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            
            let frame: RecordedFrame = serde_json::from_str(&line)?;
            frames.push((frame.ts, frame.raw_frame));
        }
        
        Ok(Self {
            frames,
            current_index: 0,
            start_time: None,
            first_frame_time: None,
            config,
            book_update_count: HashMap::new(),
            fault_applied: false,
            next_frame_buffer: None,
        })
    }

    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
        if let Some((first_ts, _)) = self.frames.first() {
            self.first_frame_time = Some(*first_ts);
        }
    }

    pub fn next_frame(&mut self) -> Option<String> {
        // Check if we have a buffered frame (from reorder fault)
        if let Some(buffered) = self.next_frame_buffer.take() {
            return Some(buffered);
        }

        if self.current_index >= self.frames.len() {
            return None;
        }
        
        let (frame_ts, mut frame_data) = self.frames[self.current_index].clone();
        
        // Check if we should wait based on replay mode
        if let Some(start) = self.start_time {
            if let Some(first_ts) = self.first_frame_time {
                let elapsed = start.elapsed();
                let frame_offset = (frame_ts - first_ts).to_std().unwrap_or_default();
                
                match self.config.mode {
                    ReplayMode::Realtime => {
                        let target_elapsed = frame_offset;
                        if elapsed < target_elapsed {
                            // Need to wait
                            return None;
                        }
                    }
                    ReplayMode::Speed(speed) => {
                        let frame_secs = frame_offset.as_secs_f64();
                        let target_secs = frame_secs / speed;
                        let target_elapsed = std::time::Duration::from_secs_f64(target_secs);
                        if elapsed < target_elapsed {
                            return None;
                        }
                    }
                    ReplayMode::AsFast => {
                        // No waiting
                    }
                }
            }
        }
        
        // Check if this is a book update frame and apply fault injection if needed
        let frame_index = self.current_index;
        let mut should_skip = false;
        
        if let Ok(mut json_value) = serde_json::from_str::<serde_json::Value>(&frame_data) {
            if let Some(channel) = json_value.get("channel").and_then(|c| c.as_str()) {
                if channel == "book" {
                    if let Some(data_array) = json_value.get("data").and_then(|d| d.as_array()) {
                        if let Some(book_data) = data_array.first() {
                            if let Some(symbol) = book_data.get("symbol").and_then(|s| s.as_str()) {
                                let count = self.book_update_count.entry(symbol.to_string()).or_insert(0);
                                *count += 1;
                                let update_index = *count;
                                
                                // Apply fault rule
                                match &self.config.fault {
                                    FaultRule::Every { n, fault } => {
                                        if update_index % n == 0 {
                                            match fault {
                                                FaultType::Drop => {
                                                    warn!("Fault injection: Dropping frame {} (book update #{}) for {}", frame_index, update_index, symbol);
                                                    should_skip = true;
                                                }
                                                FaultType::Reorder => {
                                                    if self.current_index + 1 < self.frames.len() {
                                                        warn!("Fault injection: Reordering frame {} with next (book update #{}) for {}", frame_index, update_index, symbol);
                                                        let next_frame = self.frames[self.current_index + 1].1.clone();
                                                        self.next_frame_buffer = Some(frame_data.clone());
                                                        frame_data = next_frame;
                                                        self.current_index += 1; // Skip next frame
                                                    }
                                                }
                                                FaultType::MutateQty { delta_ticks } => {
                                                    let mut json_val = json_value.clone();
                                                    if let Some(mutated) = self.mutate_qty(&mut json_val, *delta_ticks) {
                                                        warn!("Fault injection: Mutating qty in frame {} (book update #{}) for {}", frame_index, update_index, symbol);
                                                        frame_data = mutated;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    FaultRule::OnceAt { index, fault } => {
                                        if update_index == *index {
                                            match fault {
                                                FaultType::Drop => {
                                                    warn!("Fault injection: Dropping frame {} (book update #{}) for {}", frame_index, update_index, symbol);
                                                    should_skip = true;
                                                }
                                                FaultType::Reorder => {
                                                    if self.current_index + 1 < self.frames.len() {
                                                        warn!("Fault injection: Reordering frame {} with next (book update #{}) for {}", frame_index, update_index, symbol);
                                                        let next_frame = self.frames[self.current_index + 1].1.clone();
                                                        self.next_frame_buffer = Some(frame_data.clone());
                                                        frame_data = next_frame;
                                                        self.current_index += 1; // Skip next frame
                                                    }
                                                }
                                                FaultType::MutateQty { delta_ticks } => {
                                                    let mut json_val = json_value.clone();
                                                    if let Some(mutated) = self.mutate_qty(&mut json_val, *delta_ticks) {
                                                        warn!("Fault injection: Mutating qty in frame {} (book update #{}) for {}", frame_index, update_index, symbol);
                                                        frame_data = mutated;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    FaultRule::None => {}
                                }
                            }
                        }
                    }
                }
            }
        }
        
        self.current_index += 1;
        
        if should_skip {
            // Recursively call to get next frame
            return self.next_frame();
        }
        
        Some(frame_data)
    }
    
    fn mutate_qty(&self, json: &mut serde_json::Value, delta_ticks: i32) -> Option<String> {
        // Find the first qty field in bids or asks and mutate it
        if let Some(data_array) = json.get_mut("data").and_then(|d| d.as_array_mut()) {
            for book_data in data_array {
                // Try asks first
                if let Some(asks) = book_data.get_mut("asks").and_then(|a| a.as_array_mut()) {
                    if let Some(level) = asks.first_mut() {
                        if let Some(qty) = level.get_mut("qty") {
                            if let Some(qty_str) = qty.as_str() {
                                if let Ok(qty_val) = qty_str.parse::<f64>() {
                                    let increment = 1e-8; // Common increment
                                    let new_qty = (qty_val + (delta_ticks as f64 * increment)).max(0.0);
                                    *qty = serde_json::Value::String(format!("{:.8}", new_qty));
                                    return serde_json::to_string(json).ok();
                                }
                            } else if let Some(qty_num) = qty.as_f64() {
                                let increment = 1e-8;
                                let new_qty = (qty_num + (delta_ticks as f64 * increment)).max(0.0);
                                *qty = serde_json::Value::Number(serde_json::Number::from_f64(new_qty).unwrap());
                                return serde_json::to_string(json).ok();
                            }
                        }
                    }
                }
                // Try bids
                if let Some(bids) = book_data.get_mut("bids").and_then(|b| b.as_array_mut()) {
                    if let Some(level) = bids.first_mut() {
                        if let Some(qty) = level.get_mut("qty") {
                            if let Some(qty_str) = qty.as_str() {
                                if let Ok(qty_val) = qty_str.parse::<f64>() {
                                    let increment = 1e-8;
                                    let new_qty = (qty_val + (delta_ticks as f64 * increment)).max(0.0);
                                    *qty = serde_json::Value::String(format!("{:.8}", new_qty));
                                    return serde_json::to_string(json).ok();
                                }
                            } else if let Some(qty_num) = qty.as_f64() {
                                let increment = 1e-8;
                                let new_qty = (qty_num + (delta_ticks as f64 * increment)).max(0.0);
                                *qty = serde_json::Value::Number(serde_json::Number::from_f64(new_qty).unwrap());
                                return serde_json::to_string(json).ok();
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn is_done(&self) -> bool {
        self.current_index >= self.frames.len()
    }

    pub fn progress(&self) -> f64 {
        if self.frames.is_empty() {
            return 1.0;
        }
        self.current_index as f64 / self.frames.len() as f64
    }
}

