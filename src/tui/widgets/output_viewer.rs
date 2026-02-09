//! Scrollable output viewer widget.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

/// A scrollable text viewer with optional line numbers and scrollbar.
#[allow(dead_code)]
pub struct OutputViewer;

#[allow(dead_code)]
impl OutputViewer {
    /// Render a scrollable output viewer.
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to render to
    /// * `area` - The area to render within
    /// * `lines` - The lines of text to display
    /// * `scroll` - The current scroll offset (line index)
    /// * `title` - The title for the block
    pub fn render(frame: &mut Frame, area: Rect, lines: &[String], scroll: usize, title: &str) {
        Self::render_with_options(frame, area, lines, scroll, title, false)
    }

    /// Render a scrollable output viewer with line numbers.
    pub fn render_with_line_numbers(
        frame: &mut Frame,
        area: Rect,
        lines: &[String],
        scroll: usize,
        title: &str,
    ) {
        Self::render_with_options(frame, area, lines, scroll, title, true)
    }

    /// Internal render function with all options.
    fn render_with_options(
        frame: &mut Frame,
        area: Rect,
        lines: &[String],
        scroll: usize,
        title: &str,
        show_line_numbers: bool,
    ) {
        let block = Block::default()
            .title(format!(" {title} "))
            .title_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray));

        // Calculate the inner area for the scrollbar
        let inner_area = block.inner(area);

        // Format lines with optional line numbers
        let formatted_lines: Vec<Line> = lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                if show_line_numbers {
                    let line_num = format!("{:>4} ", i + 1);
                    Line::from(vec![
                        ratatui::text::Span::styled(line_num, Style::default().fg(Color::DarkGray)),
                        ratatui::text::Span::raw(line.clone()),
                    ])
                } else {
                    Line::from(line.clone())
                }
            })
            .collect();

        let paragraph = Paragraph::new(formatted_lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((scroll as u16, 0));

        frame.render_widget(paragraph, area);

        // Render scrollbar if content exceeds visible area
        let content_height = lines.len();
        let visible_height = inner_area.height as usize;

        if content_height > visible_height {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("^"))
                .end_symbol(Some("v"))
                .track_symbol(Some("|"))
                .thumb_symbol("#");

            let mut scrollbar_state =
                ScrollbarState::new(content_height.saturating_sub(visible_height)).position(scroll);

            // Render scrollbar in the inner area (right edge)
            frame.render_stateful_widget(scrollbar, inner_area, &mut scrollbar_state);
        }
    }

    /// Calculate the maximum scroll position for given content and viewport.
    pub fn max_scroll(lines: &[String], viewport_height: u16) -> usize {
        lines.len().saturating_sub(viewport_height as usize)
    }

    /// Clamp scroll position to valid range.
    pub fn clamp_scroll(scroll: usize, lines: &[String], viewport_height: u16) -> usize {
        scroll.min(Self::max_scroll(lines, viewport_height))
    }
}
