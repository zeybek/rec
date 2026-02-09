use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
};

use crate::tui::app::App;
use crate::tui::ui::render_status_bar;

// Time constants for age formatting
const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;
const WEEK: u64 = 7 * DAY;

/// Sessions list screen - the main TUI view displaying all recording sessions.
#[derive(Default)]
pub struct SessionsScreen {
    /// Table state for tracking selection (used with `TableState`)
    #[allow(dead_code)]
    table_state: TableState,
    /// Row offset where table items start (for click detection)
    /// This is: `header_height` + `table_border` + `table_header` = 3 + 1 + 1 = 5
    pub table_items_start_row: u16,
}

impl SessionsScreen {
    /// Render the sessions list screen.
    ///
    /// Layout:
    /// - Top: Title bar with filter box on the right
    /// - Middle: Sessions table
    /// - Bottom: Status bar with keyboard hints
    pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
        // Split into header, main content, and status bar
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header with filter
                Constraint::Min(0),    // Table
                Constraint::Length(1), // Status bar
            ])
            .split(area);

        // Calculate where table items start for click detection
        // table_area.y + 1 (border) + 1 (header row) = first data row
        app.sessions_screen.table_items_start_row = chunks[1].y + 2;

        Self::render_header(frame, app, chunks[0]);
        Self::render_table(frame, app, chunks[1]);
        Self::render_status(frame, chunks[2]);
    }

    /// Render the header with title and filter box.
    fn render_header(frame: &mut Frame, app: &App, area: Rect) {
        // Split header: title on left, filter on right
        let header_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(20), Constraint::Length(30)])
            .split(area);

        // Title
        let session_count = app.filtered_sessions().len();
        let total_count = app.sessions.len();
        let title = if session_count == total_count {
            format!(" Sessions ({session_count}) ")
        } else {
            format!(" Sessions ({session_count}/{total_count}) ")
        };

        let title_block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Cyan));

        frame.render_widget(title_block, header_chunks[0]);

        // Filter box
        let filter_text = if app.filter_mode {
            format!("/{}_", app.filter)
        } else if app.filter.is_empty() {
            String::from("Press / to filter")
        } else {
            format!("/{}", app.filter)
        };

        let filter_style = if app.filter_mode {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let filter_block = Paragraph::new(filter_text).style(filter_style).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Filter ")
                .border_style(if app.filter_mode {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                }),
        );

        frame.render_widget(filter_block, header_chunks[1]);
    }

    /// Render the sessions table.
    fn render_table(frame: &mut Frame, app: &App, area: Rect) {
        let sessions = app.filtered_sessions();

        // Table header
        let header_cells = ["NAME", "COMMANDS", "AGE", "TAGS"].iter().map(|h| {
            Cell::from(*h).style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
        });
        let header = Row::new(header_cells).height(1);

        // Table rows
        let rows: Vec<Row> = sessions
            .iter()
            .enumerate()
            .map(|(idx, session)| {
                let is_selected = idx == app.selected;

                let name = session.name.clone();
                let commands = session.command_count.to_string();
                let age = format_age(session.started_at);
                let tags = if session.tags.is_empty() {
                    String::from("-")
                } else {
                    session.tags.join(", ")
                };

                let style = if is_selected {
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                Row::new(vec![
                    Cell::from(name),
                    Cell::from(commands),
                    Cell::from(age),
                    Cell::from(tags),
                ])
                .style(style)
            })
            .collect();

        let widths = [
            Constraint::Min(20),    // NAME - flexible
            Constraint::Length(10), // COMMANDS
            Constraint::Length(10), // AGE
            Constraint::Min(15),    // TAGS - flexible
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .block(Block::default().borders(Borders::ALL))
            .row_highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );

        // Create table state with selection
        let mut table_state = TableState::default();
        if !sessions.is_empty() {
            table_state.select(Some(app.selected));
        }

        frame.render_stateful_widget(table, area, &mut table_state);
    }

    /// Render the status bar with keyboard hints.
    fn render_status(frame: &mut Frame, area: Rect) {
        render_status_bar(
            frame,
            area,
            &[
                ("\u{2191}\u{2193}/jk", "Navigate"),
                ("Enter", "Show"),
                ("r", "Replay"),
                ("e", "Export"),
                ("d", "Delete"),
                ("/", "Filter"),
                ("?", "Help"),
                ("q", "Quit"),
            ],
        );
    }
}

/// Format a Unix timestamp as a human-readable age string.
///
/// Returns strings like "2m ago", "3h ago", "5d ago", "2w ago".
fn format_age(timestamp: f64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    let age_secs = (now - timestamp).max(0.0) as u64;

    if age_secs < MINUTE {
        String::from("now")
    } else if age_secs < HOUR {
        let mins = age_secs / MINUTE;
        format!("{mins}m ago")
    } else if age_secs < DAY {
        let hours = age_secs / HOUR;
        format!("{hours}h ago")
    } else if age_secs < WEEK {
        let days = age_secs / DAY;
        format!("{days}d ago")
    } else {
        let weeks = age_secs / WEEK;
        format!("{weeks}w ago")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_age_now() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        assert_eq!(format_age(now), "now");
        assert_eq!(format_age(now - 30.0), "now");
    }

    #[test]
    fn test_format_age_minutes() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        assert_eq!(format_age(now - 60.0), "1m ago");
        assert_eq!(format_age(now - 120.0), "2m ago");
        assert_eq!(format_age(now - 3599.0), "59m ago");
    }

    #[test]
    fn test_format_age_hours() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        assert_eq!(format_age(now - 3600.0), "1h ago");
        assert_eq!(format_age(now - 7200.0), "2h ago");
        assert_eq!(format_age(now - 86399.0), "23h ago");
    }

    #[test]
    fn test_format_age_days() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        assert_eq!(format_age(now - 86400.0), "1d ago");
        assert_eq!(format_age(now - 172800.0), "2d ago");
        assert_eq!(format_age(now - 604799.0), "6d ago");
    }

    #[test]
    fn test_format_age_weeks() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        assert_eq!(format_age(now - 604800.0), "1w ago");
        assert_eq!(format_age(now - 1209600.0), "2w ago");
    }

    #[test]
    fn test_format_age_future_timestamp() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        // Future timestamp should show "now" (clamped to 0)
        assert_eq!(format_age(now + 1000.0), "now");
    }
}
