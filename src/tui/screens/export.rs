use std::fs;
use std::path::Path;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::export::{
    export_bash, export_circleci, export_dockerfile, export_github_action, export_gitlab_ci,
    export_makefile, export_markdown,
};
use crate::models::Session;
use crate::tui::ui::render_status_bar;

const FORMATS: &[(&str, &str)] = &[
    ("Bash Script", "Shell script with set -euo pipefail"),
    ("Makefile", "GNU Makefile with targets"),
    ("GitHub Actions", ".github/workflows/*.yml"),
    ("GitLab CI", ".gitlab-ci.yml pipeline"),
    ("Dockerfile", "Multi-stage Dockerfile"),
    ("CircleCI", ".circleci/config.yml"),
    ("Markdown", "Documentation with code blocks"),
];

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum ExportField {
    #[default]
    Format,
    OutputPath,
    Parameterize,
    Confirm,
}

#[derive(Default)]
pub struct ExportScreen {
    pub session_id: String,
    pub selected_format: usize,
    pub output_path: String,
    pub parameterize: bool,
    pub active_field: ExportField,
    /// Last error message (if any)
    pub error_message: Option<String>,
}

impl ExportScreen {
    pub fn new(session_id: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            selected_format: 0,
            output_path: String::new(),
            parameterize: false,
            active_field: ExportField::Format,
            error_message: None,
        }
    }

    pub fn next_format(&mut self) {
        if self.active_field == ExportField::Format {
            self.selected_format = (self.selected_format + 1) % FORMATS.len();
        }
    }

    pub fn prev_format(&mut self) {
        if self.active_field == ExportField::Format {
            self.selected_format = self
                .selected_format
                .checked_sub(1)
                .unwrap_or(FORMATS.len() - 1);
        }
    }

    pub fn next_field(&mut self) {
        self.active_field = match self.active_field {
            ExportField::Format => ExportField::OutputPath,
            ExportField::OutputPath => ExportField::Parameterize,
            ExportField::Parameterize => ExportField::Confirm,
            ExportField::Confirm => ExportField::Format,
        };
    }

    pub fn toggle_parameterize(&mut self) {
        if self.active_field == ExportField::Parameterize {
            self.parameterize = !self.parameterize;
        }
    }

    pub fn input_char(&mut self, c: char) {
        if self.active_field == ExportField::OutputPath {
            self.output_path.push(c);
            self.error_message = None; // Clear error on input
        }
    }

    pub fn backspace(&mut self) {
        if self.active_field == ExportField::OutputPath {
            self.output_path.pop();
        }
    }

    /// Check if we can confirm (on confirm field with valid path)
    pub fn can_confirm(&self) -> bool {
        self.active_field == ExportField::Confirm && !self.output_path.is_empty()
    }

    /// Perform the actual export to file.
    /// Returns Ok(()) on success, Err with message on failure.
    pub fn do_export(&self, session: &Session) -> Result<(), String> {
        if self.output_path.is_empty() {
            return Err("Output path is required".to_string());
        }

        // Generate content based on selected format
        let params = if self.parameterize {
            Some(
                crate::export::detect_all_parameters(session)
                    .into_iter()
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        };

        let content = match self.selected_format {
            0 => export_bash(session, params.as_deref()),
            1 => export_makefile(session, params.as_deref()),
            2 => export_github_action(session, params.as_deref()),
            3 => export_gitlab_ci(session, params.as_deref()),
            4 => export_dockerfile(session, params.as_deref()),
            5 => export_circleci(session, params.as_deref()),
            6 => export_markdown(session, params.as_deref()),
            _ => return Err("Unknown format".to_string()),
        };

        // Create parent directories if needed
        let path = Path::new(&self.output_path);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directory: {e}"))?;
            }
        }

        // Write the file
        fs::write(path, content).map_err(|e| format!("Failed to write file: {e}"))?;

        Ok(())
    }

    pub fn render(&self, frame: &mut Frame, session: &Session, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(FORMATS.len() as u16 + 2),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(area);

        // Title
        let title = Paragraph::new(Line::from(vec![
            Span::raw("Export Session: "),
            Span::styled(session.name().to_string(), Style::default().fg(Color::Cyan)),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Export Wizard"),
        );
        frame.render_widget(title, chunks[0]);

        // Format selection (radio buttons)
        let format_items: Vec<ListItem> = FORMATS
            .iter()
            .enumerate()
            .map(|(i, (name, desc))| {
                let radio = if i == self.selected_format {
                    "(●)"
                } else {
                    "( )"
                };
                let style = if i == self.selected_format {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{radio} {name}"), style),
                    Span::styled(format!(" - {desc}"), Style::default().fg(Color::DarkGray)),
                ]))
            })
            .collect();

        let format_block_style = if self.active_field == ExportField::Format {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        let format_list = List::new(format_items).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Format")
                .border_style(format_block_style),
        );
        frame.render_widget(format_list, chunks[1]);

        // Output path input
        let output_block_style = if self.active_field == ExportField::OutputPath {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        let cursor = if self.active_field == ExportField::OutputPath {
            "▌"
        } else {
            ""
        };
        let output_text = format!("{}{}", self.output_path, cursor);
        let output_paragraph = Paragraph::new(output_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Output Path")
                .border_style(output_block_style),
        );
        frame.render_widget(output_paragraph, chunks[2]);

        // Parameterize checkbox
        let param_block_style = if self.active_field == ExportField::Parameterize {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        let checkbox = if self.parameterize { "[✓]" } else { "[ ]" };
        let param_paragraph = Paragraph::new(format!("{checkbox} Enable parameterization")).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Options")
                .border_style(param_block_style),
        );
        frame.render_widget(param_paragraph, chunks[3]);

        // Confirm/Cancel buttons
        let button_style = if self.active_field == ExportField::Confirm {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let buttons = Paragraph::new(Line::from(vec![
            Span::styled("[ Confirm ]", button_style),
            Span::raw("  "),
            Span::styled("[ Cancel (Esc) ]", Style::default().fg(Color::DarkGray)),
        ]))
        .block(Block::default().borders(Borders::ALL).title("Actions"));
        frame.render_widget(buttons, chunks[4]);

        // Error message (if any)
        if let Some(ref error) = self.error_message {
            let error_paragraph = Paragraph::new(error.as_str())
                .style(Style::default().fg(Color::Red))
                .block(Block::default().borders(Borders::ALL).title(" Error "));
            frame.render_widget(error_paragraph, chunks[5]);
        }

        // Status bar
        let hints = vec![
            ("↑↓", "Select"),
            ("Tab", "Next"),
            ("Space", "Toggle"),
            ("Enter", "Confirm"),
            ("Esc", "Cancel"),
        ];
        render_status_bar(frame, chunks[6], &hints);
    }
}
