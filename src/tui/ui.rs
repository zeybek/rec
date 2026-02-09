//! Main UI rendering dispatcher.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::app::{App, ModalState, Screen};
use super::screens::SessionsScreen;
use super::widgets::{HelpPanel, Modal};

/// Main render function that dispatches to the appropriate screen.
pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Render the current screen
    match app.screen.clone() {
        Screen::Sessions => render_sessions(frame, app, area),
        Screen::Detail(id) => render_detail(frame, app, &id, area),
        Screen::Replay(id) => render_replay(frame, app, &id, area),
        Screen::Export(id) => render_export(frame, app, &id, area),
    }

    // Render modal if active (centered overlay)
    match &app.modal {
        ModalState::DeleteConfirm(id) => {
            if let Some(session) = app.sessions.iter().find(|s| s.id.to_string() == *id) {
                Modal::render_delete_confirm(frame, area, &session.name);
            }
        }
        ModalState::Help => {
            HelpPanel::render_modal(frame, area, &app.screen);
        }
        ModalState::None => {}
    }
}

/// Render the sessions list screen.
fn render_sessions(frame: &mut Frame, app: &mut App, area: Rect) {
    SessionsScreen::render(frame, app, area);
}

/// Render the session detail screen.
fn render_detail(frame: &mut Frame, app: &mut App, session_id: &str, area: Rect) {
    if let Some(session) = app.load_session(session_id) {
        app.detail_screen.render(frame, &session, area);
    } else {
        // Session not found
        let block = Block::default().title(" Error ").borders(Borders::ALL);
        let paragraph = Paragraph::new("Session not found")
            .block(block)
            .style(Style::default().fg(Color::Red));
        frame.render_widget(paragraph, area);
    }
}

/// Render the replay screen.
fn render_replay(frame: &mut Frame, app: &mut App, session_id: &str, area: Rect) {
    if let Some(session) = app.load_session(session_id) {
        app.replay_screen.render(frame, &session, area);
    } else {
        let block = Block::default().title(" Error ").borders(Borders::ALL);
        let paragraph = Paragraph::new("Session not found")
            .block(block)
            .style(Style::default().fg(Color::Red));
        frame.render_widget(paragraph, area);
    }
}

/// Render the export wizard screen.
fn render_export(frame: &mut Frame, app: &mut App, session_id: &str, area: Rect) {
    if let Some(session) = app.load_session(session_id) {
        app.export_screen.render(frame, &session, area);
    } else {
        let block = Block::default().title(" Error ").borders(Borders::ALL);
        let paragraph = Paragraph::new("Session not found")
            .block(block)
            .style(Style::default().fg(Color::Red));
        frame.render_widget(paragraph, area);
    }
}

/// Create a centered rect for modals.
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Common status bar at the bottom of screens.
pub fn render_status_bar(frame: &mut Frame, area: Rect, hints: &[(&str, &str)]) {
    let spans: Vec<Span> = hints
        .iter()
        .enumerate()
        .flat_map(|(i, (key, desc))| {
            let mut v = vec![
                Span::styled(
                    format!(" {} ", key),
                    Style::default().fg(Color::Black).bg(Color::Gray),
                ),
                Span::raw(format!(" {} ", desc)),
            ];
            if i < hints.len() - 1 {
                v.push(Span::raw(" "));
            }
            v
        })
        .collect();

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}
