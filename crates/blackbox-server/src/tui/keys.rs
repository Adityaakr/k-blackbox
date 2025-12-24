use crossterm::event::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TuiAction {
    Quit,
    ToggleRecording,
    ExportIncident,
    InjectFault,
    ReplayLastIncident,
    AcknowledgeAlert,
    MoveSelectionUp,
    MoveSelectionDown,
    SwitchTabMarket,
    SwitchTabAnalytics,
    SwitchTabIntegrity,
    SwitchTabReplay,
    ToggleHelp,
}

pub fn key_to_action(key: KeyCode) -> Option<TuiAction> {
    match key {
        KeyCode::Char('q') | KeyCode::Esc => Some(TuiAction::Quit),
        KeyCode::Char('r') | KeyCode::Char('R') => Some(TuiAction::ToggleRecording),
        KeyCode::Char('e') | KeyCode::Char('E') => Some(TuiAction::ExportIncident),
        KeyCode::Char('d') | KeyCode::Char('D') => Some(TuiAction::InjectFault),
        KeyCode::Char('p') | KeyCode::Char('P') => Some(TuiAction::ReplayLastIncident),
        KeyCode::Char('a') | KeyCode::Char('A') => Some(TuiAction::AcknowledgeAlert),
        KeyCode::Up => Some(TuiAction::MoveSelectionUp),
        KeyCode::Down => Some(TuiAction::MoveSelectionDown),
        KeyCode::Char('1') => Some(TuiAction::SwitchTabMarket),
        KeyCode::Char('2') => Some(TuiAction::SwitchTabAnalytics),
        KeyCode::Char('3') => Some(TuiAction::SwitchTabIntegrity),
        KeyCode::Char('4') => Some(TuiAction::SwitchTabReplay),
        KeyCode::Char('?') | KeyCode::Char('h') | KeyCode::Char('H') => Some(TuiAction::ToggleHelp),
        _ => None,
    }
}

