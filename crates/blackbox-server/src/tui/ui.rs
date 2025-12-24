use crate::incident::IncidentManager;
use crate::state::AppState;
use crate::tui::app::{TuiApp, TuiTab};
use crate::tui::keys::key_to_action;
use crate::tui::snapshot::UiSnapshot;
use crate::tui::widgets;
use anyhow::Context;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

pub async fn run_tui(
    mut app: TuiApp,
    mode: String,
    fault_status: String,
) -> anyhow::Result<()> {
    run_tui_with_manager(app, mode, fault_status, None).await
}

pub async fn run_tui_with_manager(
    mut app: TuiApp,
    mode: String,
    fault_status: String,
    incident_manager: Option<Arc<IncidentManager>>,
) -> anyhow::Result<()> {
    if !atty::is(atty::Stream::Stdout) {
        return Err(anyhow::anyhow!("TUI requires an interactive terminal"));
    }
    
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("Failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend).context("Failed to create terminal")?;
    
    let mut should_quit = false;
    let mut snapshot_interval = interval(Duration::from_millis(150));
    
    loop {
        // Update snapshot
        let requested_symbols = app.state.get_requested_symbols().await;
        
        // Create snapshot to get selected symbol
        let temp_snapshot = UiSnapshot::from_state(
            &app.state,
            &mode,
            app.recording_path.clone(),
            &fault_status,
            None,
            if requested_symbols.is_empty() { None } else { Some(&requested_symbols[..]) },
        ).await;
        
        let selected_symbol = app.get_selected_symbol(&temp_snapshot);
        
        // Create final snapshot with selected symbol
        let snapshot = UiSnapshot::from_state(
            &app.state,
            &mode,
            app.recording_path.clone(),
            &fault_status,
            selected_symbol.as_deref(),
            if requested_symbols.is_empty() { None } else { Some(&requested_symbols[..]) },
        ).await;
        
        // Render
        terminal.draw(|f| render_ui(f, &app, &snapshot))?;
        
        // Clear expired notifications
        if let Some((_, timestamp)) = &app.export_notification {
            if timestamp.elapsed().as_secs() >= 3 {
                app.export_notification = None;
            }
        }
        
        // Handle input
        if crossterm::event::poll(Duration::from_millis(33))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if let Some(action) = key_to_action(key.code) {
                        match action {
                            crate::tui::keys::TuiAction::ExportIncident => {
                                if let Some(ref manager) = incident_manager {
                                    match handle_export_incident(&app.state, manager).await {
                                        Ok(path) => {
                                            let short_path = path.split('/').last().unwrap_or(&path);
                                            app.export_notification = Some((format!("✓ Exported: {}", short_path), std::time::Instant::now()));
                                        }
                                        Err(e) => {
                                            tracing::error!("Export failed: {}", e);
                                            let error_msg = format!("{}", e);
                                            let short_error = if error_msg.len() > 40 {
                                                format!("{}...", &error_msg[..40])
                                            } else {
                                                error_msg
                                            };
                                            app.export_notification = Some((format!("✗ Export failed: {}", short_error), std::time::Instant::now()));
                                        }
                                    }
                                }
                            }
                            crate::tui::keys::TuiAction::ToggleRecording => {
                                handle_toggle_recording(&app.state).await;
                            }
                            crate::tui::keys::TuiAction::InjectFault => {
                                if let Some(symbol) = app.get_selected_symbol(&snapshot) {
                                    handle_fault_injection(&app.state, &symbol).await;
                                }
                            }
                            crate::tui::keys::TuiAction::ReplayLastIncident => {
                                handle_replay_incident(&app.state).await;
                            }
                            crate::tui::keys::TuiAction::MoveSelectionUp => {
                                app.move_selection_up(&snapshot);
                            }
                            crate::tui::keys::TuiAction::MoveSelectionDown => {
                                app.move_selection_down(&snapshot);
                            }
                            crate::tui::keys::TuiAction::ToggleHelp => {
                                app.show_help = !app.show_help;
                            }
                            _ => {
                                if app.handle_action(action) {
                                    should_quit = true;
                                }
                            }
                        }
                    }
                }
            }
        }
        
        if should_quit {
            break;
        }
        
        snapshot_interval.tick().await;
    }
    
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn render_ui(f: &mut Frame, app: &TuiApp, snapshot: &UiSnapshot) {
    let size = f.size();
    
    // Layout: Header | Main | Footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // Header
            Constraint::Min(0),     // Main
            Constraint::Length(1),  // Footer
        ])
        .split(size);
    
    render_header(f, chunks[0], snapshot, app);
    
    match app.current_tab {
        TuiTab::Integrity => render_integrity_tab(f, chunks[1], snapshot, app),
        _ => render_placeholder_tab(f, chunks[1], &format!("{:?} tab not implemented", app.current_tab)),
    }
    
    render_footer(f, chunks[2], app.current_tab);
    
    // Show help panel as overlay if toggled
    if app.show_help {
        let help_area = centered_rect(60, 70, size);
        widgets::render_help_panel(f, help_area);
    }
    
    // Show notification if present (expires after 3 seconds)
    if let Some((message, timestamp)) = &app.export_notification {
        let elapsed = timestamp.elapsed().as_secs();
        if elapsed < 3 {
            let notification_area = centered_rect(50, 5, size);
            let is_success = message.starts_with("✓");
            widgets::render_notification(f, notification_area, message, is_success);
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn render_header(f: &mut Frame, area: Rect, snapshot: &UiSnapshot, _app: &TuiApp) {
    let status_icon = if snapshot.connected { "●" } else { "○" };
    let status_color = if snapshot.connected { Color::Green } else { Color::Red };
    let recording_status = if snapshot.recording_path.is_some() { "ON" } else { "OFF" };
    let recording_info = if let Some(ref path) = snapshot.recording_path {
        format!("{} ({})", recording_status, 
            path.split('/').last().unwrap_or(path.as_str()))
    } else {
        recording_status.to_string()
    };
    
    let line = Line::from(vec![
        Span::styled("Kraken Blackbox — Integrity", Style::default().add_modifier(ratatui::style::Modifier::BOLD)),
        Span::raw(" │ "),
        Span::styled(snapshot.mode.clone(), Style::default().fg(Color::Cyan)),
        Span::raw(" │ "),
        Span::styled(status_icon, Style::default().fg(status_color)),
        Span::raw(" "),
        Span::styled(if snapshot.connected { "CONNECTED" } else { "DISCONNECTED" }, Style::default().fg(status_color)),
        Span::raw(" │ "),
        Span::raw(format!("Symbols: {} │ ", snapshot.symbols.len())),
        Span::raw(format!("Msg/s: {:.1} │ ", snapshot.msg_rate)),
        Span::raw(format!("Recording: {} │ ", recording_info)),
        Span::raw(format!("Fault: {}", snapshot.fault_status)),
    ]);
    
    let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));
    
    let paragraph = Paragraph::new(vec![line])
        .block(block)
        .alignment(Alignment::Left);
    
    f.render_widget(paragraph, area);
}

fn render_integrity_tab(f: &mut Frame, area: Rect, snapshot: &UiSnapshot, app: &TuiApp) {
    // Layout: Top row (Badge + Symbol Selector) | Main (Orderbook | Inspector + Incident + Events)
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(0)])
        .split(area);
    
    // Top row: Badge + Symbol Selector
    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(main_chunks[0]);
    
    widgets::render_integrity_badge(f, top_chunks[0], snapshot);
    widgets::render_symbol_selector(f, top_chunks[1], &snapshot.symbols, app.selected_symbol_index);
    
    // Main area: Orderbook + Inspector | Sidebar
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(main_chunks[1]);
    
    // Left: Orderbook (full height)
    let selected_symbol = snapshot.selected_symbol.as_deref();
    let depth = selected_symbol
        .and_then(|s| app.state.depths.get(s).map(|d| *d.value() as usize))
        .unwrap_or(10);
    widgets::render_orderbook(f, content_chunks[0], &app.state, selected_symbol, depth);
    
    // Right: Inspector + Incident + Events
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(30), Constraint::Percentage(30)])
        .split(content_chunks[1]);
    
    // Integrity Inspector
    widgets::render_integrity_inspector(f, right_chunks[0], snapshot.integrity_proof.as_ref(), selected_symbol);
    
    // Incident panel
    render_incident_panel(f, right_chunks[1], snapshot);
    
    // Event log
    widgets::render_event_log(f, right_chunks[2], &snapshot.events);
}

fn render_incident_panel(f: &mut Frame, area: Rect, snapshot: &UiSnapshot) {
    let mut lines = vec![
        Line::from("Last Incident:"),
    ];
    
    if let Some(inc) = &snapshot.last_incident {
        lines.push(Line::from(vec![
            Span::raw(format!("  ID: {}", inc.id)),
        ]));
        lines.push(Line::from(vec![
            Span::raw(format!("  Symbol: {}", inc.symbol.as_ref().map(|s| s.clone()).unwrap_or_else(|| "N/A".to_string()))),
        ]));
        lines.push(Line::from(vec![
            Span::raw(format!("  Reason: {}", inc.reason)),
        ]));
        lines.push(Line::from(vec![
            Span::raw(format!("  Time: {}", inc.timestamp.format("%H:%M:%S").to_string())),
        ]));
    } else {
        lines.push(Line::from("  (none)"));
    }
    
    lines.push(Line::from(""));
    lines.push(Line::from("Controls:"));
    lines.push(Line::from("  [R] toggle recording"));
    lines.push(Line::from("  [E] export bug bundle"));
    lines.push(Line::from("  [F] toggle fault injection"));
    lines.push(Line::from("  [A] acknowledge alert"));
    
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Incidents");
    
    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Left);
    
    f.render_widget(paragraph, area);
}

fn render_placeholder_tab(f: &mut Frame, area: Rect, message: &str) {
    let text = vec![Line::from(message)];
    let block = Block::default().borders(Borders::ALL);
    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);
    f.render_widget(paragraph, area);
}

async fn handle_toggle_recording(state: &AppState) {
    use crate::state::UiEvent;
    use blackbox_core::recorder::Recorder;
    use std::path::PathBuf;
    
    let currently_enabled = state.is_recording_enabled().await;
    
    if currently_enabled {
        // Stop recording
        let mut recorder = state.recorder.write().await;
        if let Some(ref mut rec) = *recorder {
            let _ = rec.close();
        }
        *recorder = None;
        state.set_recording_enabled(false).await;
        state.set_recording_path(None).await;
        state.push_event(UiEvent::RecordStopped).await;
        tracing::info!("Recording stopped");
    } else {
        // Start recording - generate filename
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let path = format!("recording_{}.ndjson", timestamp);
        let path_buf = PathBuf::from(&path);
        
        match Recorder::new(path_buf.clone()) {
            Ok(rec) => {
                let mut recorder = state.recorder.write().await;
                *recorder = Some(rec);
                state.set_recording_enabled(true).await;
                state.set_recording_path(Some(path.clone())).await;
                state.push_event(UiEvent::RecordStarted { path: path.clone() }).await;
                tracing::info!("Recording started: {}", path);
            }
            Err(e) => {
                tracing::error!("Failed to start recording: {}", e);
                state.push_event(UiEvent::Error(format!("Record failed: {}", e))).await;
            }
        }
    }
}

async fn handle_fault_injection(state: &AppState, symbol: &str) {
    use crate::state::UiEvent;
    
    // Trigger fault injection for this symbol
    state.fault_injector.trigger(symbol.to_string());
    
    state.push_event(UiEvent::FaultInjected { 
        fault_type: "MutateQty".to_string(), 
        symbol: symbol.to_string() 
    }).await;
}

async fn handle_replay_incident(state: &AppState) {
    use crate::state::UiEvent;
    
    if let Some(incident) = state.get_last_incident().await {
        if let Some(frames_path) = &incident.frames_path {
            // Spawn replay task
            let state_clone = state.clone();
            let path = frames_path.clone();
            tokio::spawn(async move {
                if let Err(e) = replay_incident_frames(&state_clone, &path).await {
                    tracing::error!("Replay failed: {}", e);
                }
            });
            state.push_event(UiEvent::RecordStarted { 
                path: format!("replay: {:?}", frames_path) 
            }).await;
        }
    }
}

async fn replay_incident_frames(state: &AppState, frames_path: &std::path::Path) -> anyhow::Result<()> {
    // Read NDJSON file and replay frames
    use crate::state::UiEvent;
    
    let content = tokio::fs::read_to_string(frames_path).await?;
    let lines: Vec<&str> = content.lines().collect();
    
    for (idx, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        
        // Parse NDJSON: {"ts":"...","raw_frame":"..."}
        if let Ok(_json) = serde_json::from_str::<serde_json::Value>(line) {
            // Parse and process frame
            // This would route through the same processor
            // For now, just log
            if idx % 100 == 0 {
                tracing::info!("Replay progress: {}/{}", idx, lines.len());
            }
        }
    }
    
    state.push_event(UiEvent::RecordStopped).await;
    Ok(())
}

async fn handle_export_incident(state: &AppState, manager: &Arc<IncidentManager>) -> anyhow::Result<String> {
    use crate::state::UiEvent;
    use std::io::Write;
    use zip::write::{FileOptions, ZipWriter};
    use zip::CompressionMethod;
    
    let last_incident_meta = state.get_last_incident().await;
    if let Some(inc_meta) = last_incident_meta {
        // Get frames for this symbol
        let frame_buffer = state.get_or_create_frame_buffer(&inc_meta.symbol);
        let frames: Vec<String> = frame_buffer.read().await.iter().cloned().collect();
        
        // Get integrity proof
        let proof = state.integrity_proofs.get(&inc_meta.symbol);
        
        // Create ZIP bundle
        let incidents_dir = std::path::PathBuf::from("./incidents");
        std::fs::create_dir_all(&incidents_dir)?;
        let zip_path = incidents_dir.join(format!("{}.zip", inc_meta.id));
        
        let file = std::fs::File::create(&zip_path)?;
        let mut zip = ZipWriter::new(std::io::BufWriter::new(file));
        let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
        
        // metadata.json
        zip.start_file("metadata.json", options)?;
        zip.write_all(serde_json::to_string_pretty(&inc_meta)?.as_bytes())?;
        
        // config.json
        let config = serde_json::json!({
            "symbols": state.health.iter().map(|e| e.key().clone()).collect::<Vec<_>>(),
        });
        zip.start_file("config.json", options)?;
        zip.write_all(serde_json::to_string_pretty(&config)?.as_bytes())?;
        
        // health.json
        let overall = state.overall_health();
        let health = serde_json::to_value(&overall)?;
        zip.start_file("health.json", options)?;
        zip.write_all(serde_json::to_string_pretty(&health)?.as_bytes())?;
        
        // frames.ndjson
        zip.start_file("frames.ndjson", options)?;
        for frame in &frames {
            zip.write_all(format!("{}\n", frame).as_bytes())?;
        }
        
        // checksums.json (if proof exists)
        if let Some(p) = proof {
            let checksums_json = serde_json::json!({
                "expected": p.expected_checksum,
                "computed": p.computed_checksum,
                "preview": p.checksum_preview,
                "length": p.checksum_len,
                "latency_ms": p.verify_latency_ms,
            });
            zip.start_file("checksums.json", options)?;
            zip.write_all(serde_json::to_string_pretty(&checksums_json)?.as_bytes())?;
        }
        
        zip.finish()?;
        
        // Update incident meta with zip path
        let mut updated_meta = inc_meta.clone();
        updated_meta.zip_path = Some(zip_path.clone());
        updated_meta.frames_path = Some(incidents_dir.join(format!("{}_frames.ndjson", inc_meta.id)));
        updated_meta.frame_count = frames.len();
        
        // Write frames file
        tokio::fs::write(&updated_meta.frames_path.as_ref().unwrap(), frames.join("\n")).await?;
        
        state.set_last_incident(updated_meta).await;
        state.push_event(UiEvent::IncidentExported { path: zip_path.to_string_lossy().to_string() }).await;
        
        Ok(zip_path.to_string_lossy().to_string())
    } else {
        Err(anyhow::anyhow!("No incident to export"))
    }
}

fn render_footer(f: &mut Frame, area: Rect, current_tab: TuiTab) {
    let market_style = if current_tab == TuiTab::Market {
        Style::default().fg(Color::Cyan).add_modifier(ratatui::style::Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    
    let analytics_style = if current_tab == TuiTab::Analytics {
        Style::default().fg(Color::Cyan).add_modifier(ratatui::style::Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    
    let integrity_style = if current_tab == TuiTab::Integrity {
        Style::default().fg(Color::Cyan).add_modifier(ratatui::style::Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    
    let replay_style = if current_tab == TuiTab::Replay {
        Style::default().fg(Color::Cyan).add_modifier(ratatui::style::Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    
    let line = Line::from(vec![
        Span::styled("[1] Market", market_style),
        Span::raw(" (disabled) "),
        Span::styled("[2] Analytics", analytics_style),
        Span::raw(" (disabled) "),
        Span::styled("[3] Integrity", integrity_style),
        Span::raw(" (active) "),
        Span::styled("[4] Replay", replay_style),
        Span::raw(" (disabled) │ "),
        Span::raw("[R]ecord [E]xport [D]emo [P]lay [↑↓]Select [?]Help [Q]uit"),
    ]);
    
    let block = Block::default().borders(Borders::ALL);
    let paragraph = Paragraph::new(vec![line])
        .block(block)
        .alignment(Alignment::Left);
    
    f.render_widget(paragraph, area);
}

