use crate::state::AppState;
use crate::tui::keys::TuiAction;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TuiTab {
    Market,
    Analytics,
    Integrity,
    Replay,
}

pub struct TuiApp {
    pub state: AppState,
    pub current_tab: TuiTab,
    pub recording_path: Option<String>,
    pub fault_injection_enabled: bool,
    pub alerts_acknowledged: bool,
    pub selected_symbol_index: usize, // Index into symbol list for selection
    pub show_help: bool, // Toggle help panel
    pub export_notification: Option<(String, std::time::Instant)>, // (message, timestamp)
}

impl TuiApp {
    pub fn new(state: AppState, recording_path: Option<String>) -> Self {
        Self {
            state,
            current_tab: TuiTab::Integrity, // Default to Integrity tab
            recording_path,
            fault_injection_enabled: false,
            alerts_acknowledged: false,
            selected_symbol_index: 0,
            show_help: false,
            export_notification: None,
        }
    }
    
    pub fn get_selected_symbol(&self, snapshot: &crate::tui::snapshot::UiSnapshot) -> Option<String> {
        if snapshot.symbols.is_empty() {
            None
        } else {
            let idx = if snapshot.symbols.len() > 0 {
                self.selected_symbol_index % snapshot.symbols.len()
            } else {
                0
            };
            Some(snapshot.symbols[idx].clone())
        }
    }
    
    pub fn move_selection_up(&mut self, snapshot: &crate::tui::snapshot::UiSnapshot) {
        if !snapshot.symbols.is_empty() && self.selected_symbol_index > 0 {
            self.selected_symbol_index -= 1;
        }
    }
    
    pub fn move_selection_down(&mut self, snapshot: &crate::tui::snapshot::UiSnapshot) {
        if !snapshot.symbols.is_empty() {
            self.selected_symbol_index = (self.selected_symbol_index + 1) % snapshot.symbols.len();
        }
    }
    
    pub fn handle_action(&mut self, action: TuiAction) -> bool {
        // Returns true if should quit
        match action {
            TuiAction::Quit => true,
            TuiAction::ToggleRecording => {
                // Toggle recording (for now just log, actual toggle would need state management)
                false
            }
            TuiAction::ExportIncident | TuiAction::InjectFault | TuiAction::ReplayLastIncident => {
                // These are handled in UI layer
                false
            }
            TuiAction::AcknowledgeAlert => {
                self.alerts_acknowledged = true;
                false
            }
            TuiAction::MoveSelectionUp | TuiAction::MoveSelectionDown => {
                // These are handled in UI layer
                false
            }
            TuiAction::SwitchTabMarket | 
            TuiAction::SwitchTabAnalytics | 
            TuiAction::SwitchTabReplay => {
                // Other tabs not implemented yet
                false
            }
            TuiAction::SwitchTabIntegrity => {
                self.current_tab = TuiTab::Integrity;
                false
            }
            TuiAction::ToggleHelp => {
                self.show_help = !self.show_help;
                false
            }
        }
    }
}

