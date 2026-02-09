//! Help panel widget showing keyboard shortcuts.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::tui::app::Screen;
use crate::tui::ui::centered_rect;

/// Help panel widget showing context-sensitive keyboard shortcuts.
pub struct HelpPanel;

impl HelpPanel {
    /// Render the help panel as a centered modal.
    pub fn render_modal(frame: &mut Frame, area: Rect, screen: &Screen) {
        // 50% width, 60% height centered modal
        let modal_area = centered_rect(50, 60, area);

        // Clear the area behind the modal
        frame.render_widget(Clear, modal_area);

        let block = Block::default()
            .title(" Keyboard Shortcuts ")
            .title_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let mut lines = Self::shortcuts_for_screen(screen);
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Press any key to close",
            Style::default().fg(Color::Gray),
        )));

        let paragraph = Paragraph::new(lines).block(block);

        frame.render_widget(paragraph, modal_area);
    }

    /// Get the list of shortcuts for the given screen.
    fn shortcuts_for_screen(screen: &Screen) -> Vec<Line<'static>> {
        let mut lines = vec![Line::from("")];

        match screen {
            Screen::Sessions => {
                lines.extend(Self::format_shortcuts(&[
                    (
                        "Navigation",
                        &[
                            ("j/Down", "Move down"),
                            ("k/Up", "Move up"),
                            ("g", "Go to first"),
                            ("G", "Go to last"),
                        ],
                    ),
                    (
                        "Actions",
                        &[
                            ("Enter", "View session details"),
                            ("r", "Replay session"),
                            ("e", "Export session"),
                            ("d", "Delete session"),
                        ],
                    ),
                    (
                        "Filter",
                        &[("/", "Start filter mode"), ("Esc", "Clear filter")],
                    ),
                    ("General", &[("?", "Toggle help"), ("q", "Quit")]),
                ]));
            }
            Screen::Detail(_) => {
                lines.extend(Self::format_shortcuts(&[
                    (
                        "Navigation",
                        &[("j/Down", "Scroll down"), ("k/Up", "Scroll up")],
                    ),
                    (
                        "Actions",
                        &[("r", "Replay session"), ("e", "Export session")],
                    ),
                    ("General", &[("Esc/q", "Go back"), ("?", "Toggle help")]),
                ]));
            }
            Screen::Replay(_) => {
                lines.extend(Self::format_shortcuts(&[
                    (
                        "Playback",
                        &[
                            ("Space", "Pause/Resume"),
                            ("Enter", "Run next command"),
                            ("s", "Skip current"),
                        ],
                    ),
                    (
                        "Output",
                        &[
                            ("j/Down", "Scroll output down"),
                            ("k/Up", "Scroll output up"),
                        ],
                    ),
                    (
                        "General",
                        &[("Esc/q", "Abort replay"), ("?", "Toggle help")],
                    ),
                ]));
            }
            Screen::Export(_) => {
                lines.extend(Self::format_shortcuts(&[
                    (
                        "Navigation",
                        &[
                            ("j/Down", "Next format"),
                            ("k/Up", "Previous format"),
                            ("Tab", "Next field"),
                        ],
                    ),
                    (
                        "Actions",
                        &[("Enter", "Export"), ("Space", "Toggle parameterize")],
                    ),
                    ("General", &[("Esc", "Go back"), ("?", "Toggle help")]),
                ]));
            }
        }

        lines
    }

    /// Format a list of shortcut groups into Lines.
    fn format_shortcuts(groups: &[(&str, &[(&str, &str)])]) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        for (group_name, shortcuts) in groups {
            // Group header
            lines.push(Line::from(Span::styled(
                format!(" {group_name}"),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));

            // Shortcuts
            for (key, desc) in *shortcuts {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("{key:>10}"), Style::default().fg(Color::Green)),
                    Span::raw("  "),
                    Span::styled((*desc).to_string(), Style::default().fg(Color::White)),
                ]));
            }

            lines.push(Line::from(""));
        }

        lines
    }
}
