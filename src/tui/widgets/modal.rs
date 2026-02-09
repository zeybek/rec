//! Delete confirmation modal widget.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::tui::ui::centered_rect;

/// Modal dialog widget for confirmations.
pub struct Modal;

impl Modal {
    /// Render a delete confirmation modal.
    ///
    /// Shows a centered modal with red border asking the user to confirm
    /// deletion of the specified session.
    pub fn render_delete_confirm(frame: &mut Frame, area: Rect, session_name: &str) {
        // 40% width, 20% height centered modal
        let modal_area = centered_rect(40, 20, area);

        // Clear the area behind the modal
        frame.render_widget(Clear, modal_area);

        // Build the modal content
        let title = " Delete Session ";
        let block = Block::default()
            .title(title)
            .title_style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red));

        // Body text
        let body_lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("Delete '"),
                Span::styled(
                    session_name,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("'?"),
            ]),
            Line::from(Span::styled(
                "This cannot be undone.",
                Style::default().fg(Color::Gray),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "[y]",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Yes  "),
                Span::styled(
                    "[n]",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" No"),
            ]),
        ];

        let paragraph = Paragraph::new(body_lines)
            .block(block)
            .alignment(ratatui::layout::Alignment::Center);

        frame.render_widget(paragraph, modal_area);
    }
}
