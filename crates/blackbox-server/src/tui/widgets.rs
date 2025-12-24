use crate::integrity::IntegrityProof;
use crate::state::AppState;
use crate::tui::snapshot::{IntegrityStatus, SymbolHealthRow};
use blackbox_core::orderbook::Orderbook;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Frame;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

#[derive(Clone, Copy, Debug)]
pub enum EventColor {
    Normal,
    Error,
    Warning,
    Info,
}

impl EventColor {
    pub fn to_color(self) -> Color {
        match self {
            EventColor::Normal => Color::White,
            EventColor::Error => Color::Red,
            EventColor::Warning => Color::Yellow,
            EventColor::Info => Color::Cyan,
        }
    }
}

pub fn render_integrity_badge(f: &mut Frame, area: Rect, snapshot: &crate::tui::snapshot::UiSnapshot) {
    let (status, badge_text) = snapshot.integrity_badge_status();
    
    let badge_color = match status {
        IntegrityStatus::Verified => Color::Green,
        IntegrityStatus::Degraded => Color::Yellow,
        IntegrityStatus::Broken => Color::Red,
    };
    
    let uptime_str = format_duration(snapshot.uptime_seconds);
    
    // Proof mode banner: show last event
    let last_event = snapshot.events.last().map(|e| e.text.as_str()).unwrap_or("No events");
    let event_color = snapshot.events.last()
        .map(|e| e.color.to_color())
        .unwrap_or(Color::White);
    
    let lines = vec![
        Line::from(vec![
            Span::styled(badge_text, Style::default().fg(badge_color).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Uptime: "),
            Span::styled(uptime_str, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("Incidents: "),
            Span::styled(snapshot.incident_count.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Last Event: ", Style::default().fg(Color::Yellow)),
            Span::styled(last_event, Style::default().fg(event_color)),
        ]),
        Line::from(vec![
            Span::raw("Last: "),
            Span::styled(
                snapshot.last_incident.as_ref()
                    .map(|i| i.id.clone())
                    .unwrap_or_else(|| "none".to_string()),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];
    
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Integrity Status")
        .border_style(Style::default().fg(badge_color));
    
    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(ratatui::layout::Alignment::Center);
    
    f.render_widget(paragraph, area);
}

pub fn render_integrity_table(f: &mut Frame, area: Rect, rows: &[SymbolHealthRow], selected_index: usize) {
    let table_rows: Vec<Row> = rows.iter().enumerate().map(|(idx, row)| {
        let ok_color = if row.ok_rate > 0.9999 { Color::Green } else if row.ok_rate > 0.95 { Color::Yellow } else { Color::Red };
        let has_highlight = row.consecutive_fail > 0 || row.last_mismatch.is_some();
        let is_selected = idx == selected_index;
        let bg_color = if is_selected {
            Color::Blue
        } else if has_highlight {
            Color::DarkGray
        } else {
            Color::Reset
        };
        
        Row::new(vec![
            Cell::from(row.symbol.clone()).style(Style::default().bg(bg_color)),
            Cell::from(row.checksum_ok.to_string()).style(Style::default().fg(Color::Green).bg(bg_color)),
            Cell::from(row.checksum_fail.to_string()).style(Style::default().fg(Color::Red).bg(bg_color)),
            Cell::from(format!("{:.2}%", row.ok_rate * 100.0)).style(Style::default().fg(ok_color).bg(bg_color)),
            Cell::from(row.consecutive_fail.to_string()).style(Style::default().bg(bg_color)),
            Cell::from(row.last_mismatch.as_ref().map(|s| s.clone()).unwrap_or_else(|| "-".to_string())).style(Style::default().bg(bg_color)),
            Cell::from(row.resync_count.to_string()).style(Style::default().bg(bg_color)),
            Cell::from(row.last_msg_age.map(|a| format_duration(a)).unwrap_or_else(|| "-".to_string())).style(Style::default().bg(bg_color)),
        ])
    }).collect();
    
    let table = Table::new(table_rows, [
        ratatui::layout::Constraint::Percentage(18),
        ratatui::layout::Constraint::Percentage(12),
        ratatui::layout::Constraint::Percentage(12),
        ratatui::layout::Constraint::Percentage(12),
        ratatui::layout::Constraint::Percentage(12),
        ratatui::layout::Constraint::Percentage(15),
        ratatui::layout::Constraint::Percentage(10),
        ratatui::layout::Constraint::Percentage(9),
    ])
    .header(
        Row::new(vec![
            Cell::from("Symbol"),
            Cell::from("OK"),
            Cell::from("Fail"),
            Cell::from("OK Rate"),
            Cell::from("Consec"),
            Cell::from("Last Mismatch"),
            Cell::from("Resync"),
            Cell::from("Msg Age"),
        ]).style(Style::default().add_modifier(Modifier::BOLD))
    )
    .block(Block::default().borders(Borders::ALL).title("Per-Symbol Integrity"));

    f.render_widget(table, area);
}

pub fn render_integrity_inspector(f: &mut Frame, area: Rect, proof: Option<&IntegrityProof>, symbol: Option<&str>) {
    let lines = if let Some(p) = proof {
        let status = if p.is_match() {
            ("✅ MATCH", Color::Green)
        } else {
            ("❌ MISMATCH", Color::Red)
        };
        
        vec![
            Line::from(vec![
                Span::styled("Integrity Inspector", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" - {}", symbol.unwrap_or(""))),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::Yellow)),
                Span::styled(status.0, Style::default().fg(status.1)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Expected: ", Style::default().fg(Color::Yellow)),
                Span::styled(format!("0x{:08X}", p.expected_checksum), Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled("Got: ", Style::default().fg(Color::Yellow)),
                Span::styled(format!("0x{:08X}", p.computed_checksum), Style::default().fg(if p.is_match() { Color::Green } else { Color::Red })),
            ]),
            Line::from(vec![
                Span::raw(if p.is_match() {
                    format!("  ✓ Match!")
                } else {
                    format!("  ✗ Mismatch! (diff: 0x{:08X})", p.expected_checksum.wrapping_sub(p.computed_checksum))
                }),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw(format!("Checksum Preview: {}...", &p.checksum_preview[..p.checksum_preview.len().min(64)])),
            ]),
            Line::from(vec![
                Span::raw(format!("Checksum Length: {} chars", p.checksum_len)),
            ]),
            Line::from(vec![
                Span::raw(format!("Verify Latency: {}ms", p.verify_latency_ms)),
            ]),
            Line::from(""),
            Line::from("Top 10 Asks:"),
        ]
        .into_iter()
        .chain(
            p.top_asks.iter().take(10).map(|(p, q)| {
                Line::from(format!("  {} @ {}", p, q))
            })
        )
        .chain(vec![
            Line::from(""),
            Line::from("Top 10 Bids:"),
        ])
        .chain(
            p.top_bids.iter().take(10).map(|(p, q)| {
                Line::from(format!("  {} @ {}", p, q))
            })
        )
        .chain(vec![
            Line::from(""),
            Line::from(vec![
                Span::raw(format!("Last Verify: {}", p.last_verify_ts.format("%H:%M:%S%.3f"))),
            ]),
        ])
        .chain(
            p.last_mismatch_ts.map(|ts| {
                Line::from(vec![
                    Span::raw(format!("Last Mismatch: {} ({})", ts.format("%H:%M:%S%.3f"), p.diagnosis.as_deref().unwrap_or("unknown")))
                ])
            }).into_iter()
        )
        .collect::<Vec<_>>()
    } else {
        vec![
            Line::from("Integrity Inspector"),
            Line::from(""),
            Line::from("No symbol selected"),
            Line::from("Use ↑↓ to select a symbol"),
        ]
    };
    
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Integrity Inspector");
    
    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(ratatui::layout::Alignment::Left);
    
    f.render_widget(paragraph, area);
}

pub fn render_event_log(f: &mut Frame, area: Rect, events: &[crate::state::AggregatedEvent]) {
    let log_lines: Vec<Line> = events.iter().rev().take(30).map(|entry| {
        let time_str = entry.timestamp.format("%H:%M:%S%.3f").to_string();
        Line::from(vec![
            Span::styled(format!("{} ", time_str), Style::default().fg(Color::DarkGray)),
            Span::styled(entry.text.clone(), Style::default().fg(entry.color.to_color())),
        ])
    }).collect();
    
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Last Events (most recent first)");
    
    let paragraph = Paragraph::new(log_lines)
        .block(block)
        .alignment(ratatui::layout::Alignment::Left);
    
    f.render_widget(paragraph, area);
}

pub fn render_orderbook(f: &mut Frame, area: Rect, state: &AppState, symbol: Option<&str>, depth: usize) {
    if let Some(sym) = symbol {
        if let Some(book_entry) = state.orderbooks.get(sym) {
            let book = book_entry.value();
            
            // Layout: Summary header + Orderbook (Bids | Asks)
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(5), Constraint::Min(0)])
                .split(area);
            
            // Summary header
            let best_bid = book.best_bid();
            let best_ask = book.best_ask();
            let spread = book.spread();
            let mid = book.mid();
            
            let mut summary_lines = vec![
                Line::from(vec![
                    Span::styled("Orderbook: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(sym, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                ]),
            ];
            
            if let (Some((bid_price, bid_qty)), Some((ask_price, ask_qty))) = (best_bid, best_ask) {
                summary_lines.push(Line::from(vec![
                    Span::raw("Best Bid: "),
                    Span::styled(format!("{:.2}", bid_price), Style::default().fg(Color::Green)),
                    Span::raw(format!(" @ {:.6}  │  Best Ask: ", bid_qty)),
                    Span::styled(format!("{:.2}", ask_price), Style::default().fg(Color::Red)),
                    Span::raw(format!(" @ {:.6}", ask_qty)),
                ]));
                
                if let Some(sp) = spread {
                    summary_lines.push(Line::from(vec![
                        Span::raw("Spread: "),
                        Span::styled(format!("{:.4}", sp), Style::default().fg(Color::Yellow)),
                    ]));
                    
                    if let Some(m) = mid {
                        summary_lines.push(Line::from(vec![
                            Span::raw("Mid: "),
                            Span::styled(format!("{:.4}", m), Style::default().fg(Color::Cyan)),
                        ]));
                    }
                }
            } else {
                summary_lines.push(Line::from("Waiting for orderbook data..."));
            }
            
            let summary_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan));
            
            let summary_para = Paragraph::new(summary_lines)
                .block(summary_block)
                .alignment(ratatui::layout::Alignment::Left);
            
            f.render_widget(summary_para, chunks[0]);
            
            // Orderbook: Split into Bids (left) and Asks (right)
            let orderbook_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(chunks[1]);
            
            // Calculate how many rows can fit (accounting for header and borders)
            let available_height = orderbook_chunks[0].height.saturating_sub(2); // Subtract borders
            let max_rows = available_height.saturating_sub(1); // Subtract header row
            let display_depth = depth.min(max_rows.max(10) as usize); // Use at least 10, or what fits
            
            // Get bids and asks with calculated depth
            let bids = book.bids_vec(Some(display_depth));
            let asks = book.asks_vec(Some(display_depth));
            
            // Calculate max quantity for depth bars (use all available data for scaling)
            let max_qty = bids.iter()
                .chain(asks.iter())
                .map(|(_, q)| q.to_f64().unwrap_or(0.0))
                .fold(0.0, f64::max);
            
            // Render bids (left side)
            render_orderbook_side(f, orderbook_chunks[0], "BIDS", &bids, true, max_qty, best_bid.as_ref());
            
            // Render asks (right side)
            render_orderbook_side(f, orderbook_chunks[1], "ASKS", &asks, false, max_qty, best_ask.as_ref());
        } else {
            // No orderbook data yet
            let no_data_lines = vec![
                Line::from("Orderbook"),
                Line::from(""),
                Line::from("No data available"),
                Line::from("Waiting for orderbook updates..."),
            ];
            
            let block = Block::default()
                .borders(Borders::ALL)
                .title(format!("Orderbook: {}", symbol.unwrap_or("N/A")));
            
            let paragraph = Paragraph::new(no_data_lines)
                .block(block)
                .alignment(ratatui::layout::Alignment::Center);
            
            f.render_widget(paragraph, area);
        }
    } else {
        // No symbol selected
        let no_symbol_lines = vec![
            Line::from("Orderbook"),
            Line::from(""),
            Line::from("No symbol selected"),
            Line::from("Use ↑↓ to select a symbol"),
        ];
        
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Orderbook");
        
        let paragraph = Paragraph::new(no_symbol_lines)
            .block(block)
            .alignment(ratatui::layout::Alignment::Center);
        
        f.render_widget(paragraph, area);
    }
}

fn render_orderbook_side(
    f: &mut Frame,
    area: Rect,
    title: &str,
    levels: &[(Decimal, Decimal)],
    is_bids: bool,
    max_qty: f64,
    best_level: Option<&(Decimal, Decimal)>,
) {
    let color = if is_bids { Color::Green } else { Color::Red };
    
    let mut rows = Vec::new();
    
    // Header
    rows.push(Row::new(vec![
        Cell::from("Price").style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
        Cell::from("Qty").style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
        Cell::from("Depth").style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
    ]));
    
    // Data rows
    for (price, qty) in levels.iter() {
        let price_str = format!("{:.2}", price);
        let qty_str = format!("{:.6}", qty);
        
        // Calculate depth bar width (use full available width)
        let qty_f64: f64 = qty.to_f64().unwrap_or(0.0);
        let depth_bar_width = if max_qty > 0.0 {
            // Use reasonable max width for depth bars (scale based on quantity)
            ((qty_f64 / max_qty) * 25.0) as usize
        } else {
            0
        };
        // Use block character for better visibility
        let depth_bar = if depth_bar_width > 0 {
            "█".repeat(depth_bar_width.min(25))
        } else {
            String::new()
        };
        
        // Highlight best bid/ask
        let is_best = best_level.map(|(p, _)| p == price).unwrap_or(false);
        let row_style = if is_best {
            Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        
        // Use colored depth bars - green for bids, red/pink for asks
        let depth_bar_color = if is_bids {
            Color::Green
        } else {
            Color::LightRed  // Use lighter red/pink for asks to match the visual
        };
        
        // Apply depth bar color (don't use row_style which might override)
        let depth_bar_style = Style::default().fg(depth_bar_color);
        
        rows.push(Row::new(vec![
            Cell::from(price_str.clone()).style(row_style.fg(color)),
            Cell::from(qty_str.clone()).style(row_style),
            Cell::from(depth_bar.clone()).style(depth_bar_style),
        ]));
    }
    
    if rows.len() == 1 {
        // Only header, add empty message
        rows.push(Row::new(vec![
            Cell::from("(empty)"),
            Cell::from(""),
            Cell::from(""),
        ]));
    }
    
    // Calculate column widths - give more space to depth bars
    let table = Table::new(rows, [
        Constraint::Length(12),  // Price (fixed width)
        Constraint::Length(14),  // Qty (fixed width)
        Constraint::Min(10),     // Depth bars (use remaining space)
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(color))
    );
    
    f.render_widget(table, area);
}

pub fn render_help_panel(f: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(vec![
            Span::styled("Keyboard Shortcuts", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Navigation:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from("  ↑↓    Select symbol"),
        Line::from("  1-4   Switch tabs"),
        Line::from("  ?/H   Toggle this help"),
        Line::from(""),
        Line::from(vec![
            Span::styled("Actions:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from("  R     Toggle recording"),
        Line::from("  E     Export incident bundle"),
        Line::from("  D     Inject fault (demo)"),
        Line::from("  P     Replay last incident"),
        Line::from("  A     Acknowledge alert"),
        Line::from("  Q/Esc Quit"),
        Line::from(""),
        Line::from(vec![
            Span::styled("Tabs:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from("  [1] Market      - Orderbook view"),
        Line::from("  [2] Analytics   - Statistics & charts"),
        Line::from("  [3] Integrity   - Checksum verification"),
        Line::from("  [4] Replay      - Incident replay"),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press ? or H to close", Style::default().fg(Color::DarkGray)),
        ]),
    ];
    
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Help")
        .border_style(Style::default().fg(Color::Cyan));
    
    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(ratatui::layout::Alignment::Left);
    
    f.render_widget(paragraph, area);
}

pub fn render_notification(f: &mut Frame, area: Rect, message: &str, is_success: bool) {
    let color = if is_success { Color::Green } else { Color::Red };
    let icon = if is_success { "✓" } else { "✗" };
    
    let lines = vec![
        Line::from(vec![
            Span::styled(icon, Style::default().fg(color)),
            Span::raw(" "),
            Span::styled(message, Style::default().fg(color)),
        ]),
    ];
    
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color))
        .style(Style::default().bg(Color::Black));
    
    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(ratatui::layout::Alignment::Center);
    
    f.render_widget(paragraph, area);
}

pub fn render_symbol_selector(f: &mut Frame, area: Rect, symbols: &[String], selected_index: usize) {
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Symbols", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" (↑↓ to select)"),
        ]),
        Line::from(""),
    ];
    
    for (idx, symbol) in symbols.iter().enumerate() {
        let is_selected = idx == selected_index;
        let prefix = if is_selected {
            "▶ "
        } else {
            "  "
        };
        
        let style = if is_selected {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        
        lines.push(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(symbol.clone(), style),
        ]));
    }
    
    if symbols.is_empty() {
        lines.push(Line::from("  (no symbols)"));
    }
    
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Symbol Selector");
    
    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(ratatui::layout::Alignment::Left);
    
    f.render_widget(paragraph, area);
}

fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    }
}
