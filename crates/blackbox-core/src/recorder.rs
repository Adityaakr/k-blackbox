use crate::types::RecordedFrame;
use chrono::Utc;
use serde_json;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

pub struct Recorder {
    writer: Option<BufWriter<File>>,
    path: PathBuf,
}

impl Recorder {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let file = File::create(&path)?;
        let writer = BufWriter::new(file);
        
        Ok(Self {
            writer: Some(writer),
            path,
        })
    }

    pub fn record_frame(&mut self, raw_frame: &str, decoded_event: Option<&str>) -> anyhow::Result<()> {
        if let Some(writer) = &mut self.writer {
            let frame = RecordedFrame {
                ts: Utc::now(),
                raw_frame: raw_frame.to_string(),
                decoded_event: decoded_event.map(|s| s.to_string()),
            };
            
            let json = serde_json::to_string(&frame)?;
            writeln!(writer, "{}", json)?;
            writer.flush()?;
        }
        
        Ok(())
    }

    pub fn close(&mut self) -> anyhow::Result<()> {
        if let Some(writer) = &mut self.writer {
            writer.flush()?;
        }
        self.writer = None;
        Ok(())
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

