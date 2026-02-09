//! Session detail screen.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Table,
    },
};

use crate::models::Session;
use crate::tui::ui::render_status_bar;

#[derive(Default)]
pub struct DetailScreen {
    pub scroll: usize,
    pub session_id: String,
    /// Command count for scroll bounds (set during render)
    command_count: usize,
    /// Visible height for scroll bounds (set during render)
    visible_height: usize,
}

impl DetailScreen {
    pub fn new(session_id: &str) -> Self {
        Self {
            scroll: 0,
            session_id: session_id.to_string(),
            command_count: 0,
            visible_height: 10,
        }
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        let max_scroll = self.command_count.saturating_sub(self.visible_height);
        if self.scroll < max_scroll {
            self.scroll = self.scroll.saturating_add(1);
        }
    }

    pub fn render(&mut self, frame: &mut Frame, session: &Session, area: Rect) {
        // Update bounds for scroll clamping
        self.command_count = session.commands.len();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(area);

        Self::render_header(frame, session, chunks[0]);
        self.render_commands(frame, session, chunks[1]);
        render_status_bar(
            frame,
            chunks[2],
            &[
                ("↑↓/jk", "Scroll"),
                ("r", "Replay"),
                ("e", "Export"),
                ("Esc", "Back"),
                ("?", "Help"),
            ],
        );
    }

    fn render_header(frame: &mut Frame, session: &Session, area: Rect) {
        let duration = session.footer.as_ref().map_or_else(
            || "-".to_string(),
            |f| {
                let ms = ((f.ended_at - session.header.started_at) * 1000.0) as u64;
                format_duration(ms)
            },
        );

        let header_text = vec![
            Line::from(vec![
                Span::styled("ID: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(session.header.id.to_string()),
            ]),
            Line::from(vec![
                Span::styled("Shell: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&session.header.shell),
            ]),
            Line::from(vec![
                Span::styled("Duration: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(duration),
            ]),
            Line::from(vec![
                Span::styled("Commands: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(session.commands.len().to_string()),
            ]),
        ];

        let header = Paragraph::new(header_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Session Details "),
        );

        frame.render_widget(header, area);
    }

    fn render_commands(&mut self, frame: &mut Frame, session: &Session, area: Rect) {
        let header_cells = ["#", "COMMAND", "EXIT", "DURATION"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD)));
        let header = Row::new(header_cells).height(1);

        let visible_height = area.height.saturating_sub(3) as usize;
        self.visible_height = visible_height; // Update for scroll bounds
        let max_scroll = session.commands.len().saturating_sub(visible_height);
        let scroll = self.scroll.min(max_scroll);

        let rows: Vec<Row> = session
            .commands
            .iter()
            .enumerate()
            .skip(scroll)
            .take(visible_height)
            .map(|(idx, cmd)| {
                let exit_code = cmd.exit_code.unwrap_or(0);
                let duration = cmd
                    .duration_ms
                    .map_or_else(|| "-".to_string(), format_duration);

                let style = if exit_code != 0 {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default()
                };

                Row::new(vec![
                    Cell::from(format!("{}", idx + 1)),
                    Cell::from(cmd.command.clone()),
                    Cell::from(format!("{exit_code}")),
                    Cell::from(duration),
                ])
                .style(style)
            })
            .collect();

        let widths = [
            Constraint::Length(4),
            Constraint::Min(20),
            Constraint::Length(6),
            Constraint::Length(10),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .block(Block::default().borders(Borders::ALL).title(" Commands "));

        frame.render_widget(table, area);

        if session.commands.len() > visible_height {
            let mut scrollbar_state = ScrollbarState::new(session.commands.len()).position(scroll);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));

            let scrollbar_area = Rect {
                x: area.x + area.width.saturating_sub(1),
                y: area.y + 1,
                width: 1,
                height: area.height.saturating_sub(2),
            };

            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }
}

fn format_duration(millis: u64) -> String {
    if millis >= 1000 {
        format!("{:.1}s", millis as f64 / 1000.0)
    } else {
        format!("{millis}ms")
    }
}
