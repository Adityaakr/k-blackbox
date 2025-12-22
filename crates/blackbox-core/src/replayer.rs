use crate::types::{RecordedFrame, ReplayConfig, ReplayMode};
use chrono::{DateTime, Utc};
use serde_json;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::Instant;

pub struct Replayer {
    frames: Vec<(DateTime<Utc>, String)>,
    current_index: usize,
    start_time: Option<Instant>,
    first_frame_time: Option<DateTime<Utc>>,
    config: ReplayConfig,
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
        })
    }

    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
        if let Some((first_ts, _)) = self.frames.first() {
            self.first_frame_time = Some(*first_ts);
        }
    }

    pub fn next_frame(&mut self) -> Option<String> {
        if self.current_index >= self.frames.len() {
            return None;
        }
        
        let (frame_ts, frame_data) = &self.frames[self.current_index];
        
        // Check if we should wait based on replay mode
        if let Some(start) = self.start_time {
            if let Some(first_ts) = self.first_frame_time {
                let elapsed = start.elapsed();
                let frame_offset = (*frame_ts - first_ts).to_std().unwrap_or_default();
                
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
        
        self.current_index += 1;
        Some(frame_data.clone())
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

