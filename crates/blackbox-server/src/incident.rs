use blackbox_core::incident::{Incident, IncidentMetadata, IncidentReason};
use blackbox_core::types::InstrumentInfo;
use chrono::{DateTime, Utc};
use serde_json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use zip::{ZipWriter, write::FileOptions, CompressionMethod};
use std::io::Write;

#[derive(Clone)]
pub struct IncidentManager {
    incidents: Arc<RwLock<Vec<Incident>>>,
    last_incident: Arc<RwLock<Option<Incident>>>,
    incidents_dir: PathBuf,
}

impl IncidentManager {
    pub fn new(incidents_dir: PathBuf) -> anyhow::Result<Self> {
        // Create incidents directory if needed
        if let Some(parent) = incidents_dir.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::create_dir_all(&incidents_dir)?;
        
        Ok(Self {
            incidents: Arc::new(RwLock::new(Vec::new())),
            last_incident: Arc::new(RwLock::new(None)),
            incidents_dir,
        })
    }

    pub async fn record_incident(
        &self,
        reason: IncidentReason,
        symbol: Option<String>,
        metadata: serde_json::Value,
    ) -> Incident {
        let incident = Incident::new(reason, symbol.clone())
            .with_metadata(metadata);
        
        {
            let mut incidents = self.incidents.write().await;
            incidents.push(incident.clone());
        }
        
        {
            let mut last = self.last_incident.write().await;
            *last = Some(incident.clone());
        }
        
        tracing::warn!("Incident recorded: {} - {:?} for {:?}", incident.id, incident.reason, symbol);
        
        incident
    }

    pub async fn get_last_incident(&self) -> Option<Incident> {
        self.last_incident.read().await.clone()
    }

    pub async fn export_incident_bundle(
        &self,
        incident: &Incident,
        config: serde_json::Value,
        health: serde_json::Value,
        instrument: Option<&InstrumentInfo>,
        book_top: Option<serde_json::Value>,
        frames: &[(DateTime<Utc>, String)],
        incident_time: DateTime<Utc>,
    ) -> anyhow::Result<PathBuf> {
        let bundle_path = self.incidents_dir.join(format!("{}.zip", incident.id));
        
        let file = std::fs::File::create(&bundle_path)?;
        let mut zip = ZipWriter::new(std::io::BufWriter::new(file));
        let options = FileOptions::default()
            .compression_method(CompressionMethod::Deflated);

        // Write metadata.json
        let metadata = IncidentMetadata {
            incident: incident.clone(),
            config: config.clone(),
            health: health.clone(),
            instrument: instrument.map(|i| serde_json::to_value(i).unwrap()),
            book_top,
        };
        zip.start_file("metadata.json", options)?;
        zip.write_all(serde_json::to_string_pretty(&metadata)?.as_bytes())?;

        // Write config.json
        zip.start_file("config.json", options)?;
        zip.write_all(serde_json::to_string_pretty(&config)?.as_bytes())?;

        // Write health.json
        zip.start_file("health.json", options)?;
        zip.write_all(serde_json::to_string_pretty(&health)?.as_bytes())?;

        // Write instrument.json (if available)
        if let Some(inst) = instrument {
            zip.start_file("instrument.json", options)?;
            zip.write_all(serde_json::to_string_pretty(inst)?.as_bytes())?;
        }

        // Write book_top.json (if available)
        if let Some(bt) = &metadata.book_top {
            zip.start_file("book_top.json", options)?;
            zip.write_all(serde_json::to_string_pretty(bt)?.as_bytes())?;
        }

        // Write frames.ndjson (t-30s to t+5s around incident)
        let window_start = incident_time - chrono::Duration::seconds(30);
        let window_end = incident_time + chrono::Duration::seconds(5);
        let relevant_frames: Vec<_> = frames
            .iter()
            .filter(|(ts, _)| *ts >= window_start && *ts <= window_end)
            .collect();

        zip.start_file("frames.ndjson", options)?;
        for (ts, frame) in relevant_frames {
            let line = format!("{{\"ts\":\"{}\",\"raw_frame\":{}}}\n", ts.to_rfc3339(), frame);
            zip.write_all(line.as_bytes())?;
        }

        zip.finish()?;
        
        tracing::info!("Incident bundle exported: {:?}", bundle_path);
        Ok(bundle_path)
    }

    pub fn incidents_dir(&self) -> &Path {
        &self.incidents_dir
    }
}

