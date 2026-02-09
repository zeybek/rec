use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Gauge, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
};

use crate::models::Session;
use crate::tui::ui::render_status_bar;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CommandState {
    #[default]
    Pending,
    Running,
    Done(i32),
    Skipped,
}

#[derive(Default)]
pub struct ReplayScreen {
    pub session_id: String,
    pub current_index: usize,
    pub paused: bool,
    pub output_lines: Vec<String>,
    pub output_scroll: usize,
    pub command_states: Vec<CommandState>,
    /// Viewport height for output scroll clamping (set during render)
    viewport_height: usize,
}

impl ReplayScreen {
    pub fn new(session_id: &str, command_count: usize) -> Self {
        // Initialize command states - first one is Running, rest are Pending
        let command_states = if command_count == 0 {
            Vec::new()
        } else {
            let mut states = vec![CommandState::Pending; command_count];
            states[0] = CommandState::Running;
            states
        };

        Self {
            session_id: session_id.to_string(),
            current_index: 0,
            paused: true,
            output_lines: Vec::new(),
            output_scroll: 0,
            command_states,
            viewport_height: 10, // Default, updated during render
        }
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub fn skip_current(&mut self) {
        if self.current_index < self.command_states.len() {
            self.command_states[self.current_index] = CommandState::Skipped;
            self.current_index += 1;
            if self.current_index < self.command_states.len() {
                self.command_states[self.current_index] = CommandState::Running;
            }
        }
    }

    pub fn run_next(&mut self) {
        if self.current_index < self.command_states.len() {
            // Mark current as done with success (actual execution will set real exit code)
            self.command_states[self.current_index] = CommandState::Done(0);
            self.current_index += 1;
            if self.current_index < self.command_states.len() {
                self.command_states[self.current_index] = CommandState::Running;
            }
        }
    }

    pub fn scroll_output_up(&mut self) {
        self.output_scroll = self.output_scroll.saturating_sub(1);
    }

    pub fn scroll_output_down(&mut self) {
        if !self.output_lines.is_empty() {
            let max_scroll = self.output_lines.len().saturating_sub(self.viewport_height);
            if self.output_scroll < max_scroll {
                self.output_scroll = self.output_scroll.saturating_add(1);
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame, session: &Session, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Progress bar
                Constraint::Min(5),    // Main content
                Constraint::Length(1), // Status bar
            ])
            .split(area);

        self.render_progress_bar(frame, session, chunks[0]);
        self.render_main_content(frame, session, chunks[1]);
        self.render_replay_status_bar(frame, chunks[2]);
    }

    fn render_progress_bar(&self, frame: &mut Frame, session: &Session, area: Rect) {
        let total = session.commands.len();
        let completed = self
            .command_states
            .iter()
            .filter(|s| matches!(s, CommandState::Done(_) | CommandState::Skipped))
            .count();

        let ratio = if total == 0 {
            0.0
        } else {
            completed as f64 / total as f64
        };

        let label = format!("{completed}/{total} commands");

        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("Progress"))
            .gauge_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .ratio(ratio)
            .label(label);

        frame.render_widget(gauge, area);
    }

    fn render_main_content(&mut self, frame: &mut Frame, session: &Session, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40), // Command list
                Constraint::Percentage(60), // Output viewer
            ])
            .split(area);

        self.render_command_list(frame, session, chunks[0]);
        self.render_output_viewer(frame, chunks[1]);
    }

    fn render_command_list(&self, frame: &mut Frame, session: &Session, area: Rect) {
        let items: Vec<ListItem> = session
            .commands
            .iter()
            .enumerate()
            .map(|(i, cmd)| {
                let state = self.command_states.get(i).unwrap_or(&CommandState::Pending);
                let (icon, style) = match state {
                    CommandState::Pending => ("○", Style::default().fg(Color::DarkGray)),
                    CommandState::Running => (
                        "▸",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    CommandState::Done(code) => {
                        if *code == 0 {
                            ("✓", Style::default().fg(Color::Green))
                        } else {
                            ("✗", Style::default().fg(Color::Red))
                        }
                    }
                    CommandState::Skipped => ("⊘", Style::default().fg(Color::DarkGray)),
                };

                let content = Line::from(vec![
                    Span::styled(format!("{icon} "), style),
                    Span::styled(
                        truncate_command(&cmd.command, area.width.saturating_sub(6) as usize),
                        style,
                    ),
                ]);

                ListItem::new(content)
            })
            .collect();

        let title = if self.paused {
            "Commands [PAUSED]"
        } else {
            "Commands"
        };

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_widget(list, area);
    }

    fn render_output_viewer(&mut self, frame: &mut Frame, area: Rect) {
        let inner_height = area.height.saturating_sub(2) as usize;
        self.viewport_height = inner_height; // Update for scroll clamping
        let max_scroll = self.output_lines.len().saturating_sub(inner_height);
        let scroll = self.output_scroll.min(max_scroll);

        let visible_lines: Vec<Line> = self
            .output_lines
            .iter()
            .skip(scroll)
            .take(inner_height)
            .map(|line| Line::from(line.as_str()))
            .collect();

        let paragraph = Paragraph::new(visible_lines)
            .block(Block::default().borders(Borders::ALL).title("Output"))
            .style(Style::default().fg(Color::White));

        frame.render_widget(paragraph, area);

        // Render scrollbar if needed
        if self.output_lines.len() > inner_height {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));

            let mut scrollbar_state = ScrollbarState::new(self.output_lines.len())
                .position(scroll)
                .viewport_content_length(inner_height);

            let scrollbar_area = Rect {
                x: area.x + area.width.saturating_sub(1),
                y: area.y + 1,
                width: 1,
                height: area.height.saturating_sub(2),
            };

            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }

    fn render_replay_status_bar(&self, frame: &mut Frame, area: Rect) {
        let pause_hint = if self.paused {
            ("Space", "Resume")
        } else {
            ("Space", "Pause")
        };
        let hints: &[(&str, &str)] = &[
            pause_hint,
            ("s", "Skip"),
            ("Enter", "Next"),
            ("a", "Abort"),
            ("^|v", "Scroll"),
            ("?", "Help"),
        ];
        render_status_bar(frame, area, hints);
    }
}

fn truncate_command(cmd: &str, max_len: usize) -> String {
    if cmd.len() <= max_len {
        cmd.to_string()
    } else if max_len > 3 {
        format!("{}...", &cmd[..max_len - 3])
    } else {
        cmd.chars().take(max_len).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_screen_new() {
        let screen = ReplayScreen::new("test-session", 3);
        assert_eq!(screen.session_id, "test-session");
        assert_eq!(screen.current_index, 0);
        assert!(screen.paused);
        assert!(screen.output_lines.is_empty());
        assert_eq!(screen.output_scroll, 0);
        assert_eq!(screen.command_states.len(), 3);
        assert_eq!(screen.command_states[0], CommandState::Running);
        assert_eq!(screen.command_states[1], CommandState::Pending);
        assert_eq!(screen.command_states[2], CommandState::Pending);
    }

    #[test]
    fn test_replay_screen_new_empty() {
        let screen = ReplayScreen::new("test-session", 0);
        assert!(screen.command_states.is_empty());
    }

    #[test]
    fn test_toggle_pause() {
        let mut screen = ReplayScreen::new("test", 1);
        assert!(screen.paused);
        screen.toggle_pause();
        assert!(!screen.paused);
        screen.toggle_pause();
        assert!(screen.paused);
    }

    #[test]
    fn test_skip_current() {
        let mut screen = ReplayScreen::new("test", 3);
        // First command is already Running

        screen.skip_current();
        assert_eq!(screen.command_states[0], CommandState::Skipped);
        assert_eq!(screen.command_states[1], CommandState::Running);
        assert_eq!(screen.current_index, 1);
    }

    #[test]
    fn test_run_next() {
        let mut screen = ReplayScreen::new("test", 3);
        // First command is already Running

        screen.run_next();
        assert_eq!(screen.command_states[0], CommandState::Done(0));
        assert_eq!(screen.command_states[1], CommandState::Running);
        assert_eq!(screen.current_index, 1);
    }

    #[test]
    fn test_scroll_output_with_bounds() {
        let mut screen = ReplayScreen::new("test", 1);
        screen.viewport_height = 2; // Can see 2 lines at a time
        screen.output_lines = vec![
            "line1".to_string(),
            "line2".to_string(),
            "line3".to_string(),
            "line4".to_string(),
        ];

        assert_eq!(screen.output_scroll, 0);
        screen.scroll_output_down();
        assert_eq!(screen.output_scroll, 1);
        screen.scroll_output_down();
        assert_eq!(screen.output_scroll, 2); // max_scroll = 4 - 2 = 2
        screen.scroll_output_down();
        assert_eq!(screen.output_scroll, 2); // Should not exceed max
        screen.scroll_output_up();
        assert_eq!(screen.output_scroll, 1);
        screen.scroll_output_up();
        assert_eq!(screen.output_scroll, 0);
        screen.scroll_output_up();
        assert_eq!(screen.output_scroll, 0); // Should not go negative
    }

    #[test]
    fn test_scroll_output_empty() {
        let mut screen = ReplayScreen::new("test", 1);
        // Empty output lines
        screen.scroll_output_down();
        assert_eq!(screen.output_scroll, 0);
    }

    #[test]
    fn test_skip_at_end() {
        let mut screen = ReplayScreen::new("test", 1);
        // First command is already Running

        screen.skip_current();
        assert_eq!(screen.command_states[0], CommandState::Skipped);
        assert_eq!(screen.current_index, 1);

        // Should not panic when at end
        screen.skip_current();
        assert_eq!(screen.current_index, 1);
    }

    #[test]
    fn test_run_next_at_end() {
        let mut screen = ReplayScreen::new("test", 1);
        // First command is already Running

        screen.run_next();
        assert_eq!(screen.command_states[0], CommandState::Done(0));
        assert_eq!(screen.current_index, 1);

        // Should not panic when at end
        screen.run_next();
        assert_eq!(screen.current_index, 1);
    }

    #[test]
    fn test_truncate_command() {
        assert_eq!(truncate_command("short", 10), "short");
        assert_eq!(truncate_command("this is a long command", 10), "this is...");
        assert_eq!(truncate_command("abc", 3), "abc");
        assert_eq!(truncate_command("abcd", 3), "abc");
    }

    #[test]
    fn test_command_state_default() {
        let state: CommandState = CommandState::default();
        assert_eq!(state, CommandState::Pending);
    }
}
