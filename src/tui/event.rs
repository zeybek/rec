//! Event handling for keyboard and mouse input.

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use std::time::Duration;

/// Actions that can be triggered by user input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Move selection up
    Up,
    /// Move selection down
    Down,
    /// Move to first item
    First,
    /// Move to last item
    Last,
    /// Confirm/Enter selection
    Enter,
    /// Go back / Cancel
    Back,
    /// Quit the application
    Quit,
    /// Delete selected item
    Delete,
    /// Replay selected session
    Replay,
    /// Export selected session
    Export,
    /// Show help panel
    Help,
    /// Toggle filter/search mode
    Filter,
    /// Scroll output up
    ScrollUp,
    /// Scroll output down
    ScrollDown,
    /// Pause/Resume replay
    Pause,
    /// Skip current command in replay
    Skip,
    /// Abort replay
    Abort,
    /// Tab to next field
    Tab,
    /// Toggle checkbox/option
    Toggle,
    /// Character input for filter
    Char(char),
    /// Backspace in filter
    Backspace,
    /// Mouse click at position
    Click(u16, u16),
    /// Mouse scroll
    Scroll(i16),
    /// No action
    None,
}

/// Poll for events with a timeout.
/// Returns None if no event occurred within the timeout.
pub fn poll_event(timeout: Duration) -> Option<Event> {
    if event::poll(timeout).ok()? {
        event::read().ok()
    } else {
        None
    }
}

/// Convert a crossterm event to an Action.
/// `text_input_mode` should be true when in filter mode or text input fields.
pub fn handle_event(event: &Event, text_input_mode: bool) -> Action {
    match event {
        Event::Key(key) => handle_key_event(*key, text_input_mode),
        Event::Mouse(mouse) => handle_mouse_event(*mouse),
        _ => Action::None,
    }
}

/// Handle keyboard events.
fn handle_key_event(key: KeyEvent, text_input_mode: bool) -> Action {
    // If in text input mode (filter or text field), most keys are character input
    if text_input_mode {
        return match key.code {
            KeyCode::Esc => Action::Back,
            KeyCode::Enter => Action::Enter,
            KeyCode::Backspace => Action::Backspace,
            KeyCode::Tab => Action::Tab, // Allow tab to switch fields
            KeyCode::Up => Action::Up,   // Allow navigation in export screen
            KeyCode::Down => Action::Down,
            KeyCode::Char(c) => Action::Char(c),
            _ => Action::None,
        };
    }

    // Normal mode key handling
    match key.code {
        // Navigation - Arrow keys
        KeyCode::Up | KeyCode::Char('k') => Action::Up,
        KeyCode::Down | KeyCode::Char('j') => Action::Down,
        KeyCode::Home | KeyCode::Char('g') => Action::First,
        KeyCode::End | KeyCode::Char('G') => Action::Last,
        KeyCode::PageUp => Action::ScrollUp,
        KeyCode::PageDown => Action::ScrollDown,

        // Actions
        KeyCode::Enter => Action::Enter,
        KeyCode::Esc => Action::Back,
        KeyCode::Tab => Action::Tab,
        KeyCode::Char(' ') => Action::Toggle,

        // Quit
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::Quit,

        // Session actions
        KeyCode::Char('d') => Action::Delete,
        KeyCode::Char('r') => Action::Replay,
        KeyCode::Char('e') => Action::Export,
        KeyCode::Char('/') => Action::Filter,
        KeyCode::Char('?') => Action::Help,

        // Replay actions
        KeyCode::Char('s') => Action::Skip,
        KeyCode::Char('a') => Action::Abort,
        KeyCode::Char('p') => Action::Pause,

        _ => Action::None,
    }
}

/// Handle mouse events.
fn handle_mouse_event(mouse: MouseEvent) -> Action {
    match mouse.kind {
        MouseEventKind::Down(_) => Action::Click(mouse.column, mouse.row),
        MouseEventKind::ScrollUp => Action::Scroll(-1),
        MouseEventKind::ScrollDown => Action::Scroll(1),
        _ => Action::None,
    }
}
