//! Main application state and event loop.

use crate::error::Result;
use crate::models::Session;
use crate::storage::{Paths, SessionStore};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, prelude::CrosstermBackend};
use std::io::{self, Stdout};
use std::time::Duration;
use uuid::Uuid;

use super::event::{self, Action};
use super::screens::{DetailScreen, ExportScreen, ReplayScreen, SessionsScreen};
use super::ui;

/// Simplified session info for TUI display.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// Session UUID
    pub id: Uuid,
    /// Session name
    pub name: String,
    /// Shell used
    pub shell: String,
    /// Tags
    pub tags: Vec<String>,
    /// Number of commands
    pub command_count: usize,
    /// Session start timestamp
    pub started_at: f64,
    /// Duration in seconds (if completed)
    pub duration_secs: Option<u64>,
}

/// The current screen being displayed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    /// Session list (main screen)
    Sessions,
    /// Session detail view
    Detail(String), // session_id
    /// Replay view
    Replay(String), // session_id
    /// Export wizard
    Export(String), // session_id
}

/// Modal dialog state.
#[derive(Debug, Clone)]
pub enum ModalState {
    /// No modal visible
    None,
    /// Delete confirmation
    DeleteConfirm(String), // session_id
    /// Help panel visible
    Help,
}

/// Main application state.
pub struct App {
    /// Current screen
    pub screen: Screen,
    /// Modal state
    pub modal: ModalState,
    /// Should quit the application
    pub should_quit: bool,
    /// Session store
    pub store: SessionStore,
    /// Cached session info
    pub sessions: Vec<SessionInfo>,
    /// Selected session index
    pub selected: usize,
    /// Filter text
    pub filter: String,
    /// Is filter mode active
    pub filter_mode: bool,
    /// Sessions screen state
    pub sessions_screen: SessionsScreen,
    /// Detail screen state
    pub detail_screen: DetailScreen,
    /// Replay screen state
    pub replay_screen: ReplayScreen,
    /// Export screen state
    pub export_screen: ExportScreen,
}

impl App {
    /// Create a new App instance.
    pub fn new(paths: &Paths) -> Result<Self> {
        let store = SessionStore::new(paths.clone());
        let sessions = Self::load_sessions(&store);

        Ok(Self {
            screen: Screen::Sessions,
            modal: ModalState::None,
            should_quit: false,
            store,
            sessions,
            selected: 0,
            filter: String::new(),
            filter_mode: false,
            sessions_screen: SessionsScreen::default(),
            detail_screen: DetailScreen::default(),
            replay_screen: ReplayScreen::default(),
            export_screen: ExportScreen::default(),
        })
    }

    /// Load sessions from store into SessionInfo structs.
    fn load_sessions(store: &SessionStore) -> Vec<SessionInfo> {
        let session_ids = store.list().unwrap_or_default();
        let mut sessions = Vec::new();

        for id in session_ids {
            if let Ok(session) = store.load(&id) {
                let duration_secs = session
                    .footer
                    .as_ref()
                    .map(|f| (f.ended_at - session.header.started_at).max(0.0) as u64);

                sessions.push(SessionInfo {
                    id: session.header.id,
                    name: session.header.name,
                    shell: session.header.shell,
                    tags: session.header.tags,
                    command_count: session.commands.len(),
                    started_at: session.header.started_at,
                    duration_secs,
                });
            }
        }

        // Sort by started_at descending (newest first)
        sessions.sort_by(|a, b| {
            b.started_at
                .partial_cmp(&a.started_at)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        sessions
    }

    /// Refresh the session list.
    pub fn refresh_sessions(&mut self) {
        self.sessions = Self::load_sessions(&self.store);

        // Clamp selected index
        if self.selected >= self.sessions.len() && !self.sessions.is_empty() {
            self.selected = self.sessions.len() - 1;
        }
    }

    /// Get the filtered sessions based on current filter.
    pub fn filtered_sessions(&self) -> Vec<&SessionInfo> {
        if self.filter.is_empty() {
            self.sessions.iter().collect()
        } else {
            let filter_lower = self.filter.to_lowercase();
            self.sessions
                .iter()
                .filter(|s| {
                    s.name.to_lowercase().contains(&filter_lower)
                        || s.tags
                            .iter()
                            .any(|t| t.to_lowercase().contains(&filter_lower))
                })
                .collect()
        }
    }

    /// Get currently selected session.
    pub fn selected_session(&self) -> Option<&SessionInfo> {
        let filtered = self.filtered_sessions();
        filtered.get(self.selected).copied()
    }

    /// Load full session by ID.
    pub fn load_session(&self, id: &str) -> Option<Session> {
        self.store.load(id).ok()
    }

    /// Delete a session by ID.
    pub fn delete_session(&mut self, id: &str) -> Result<()> {
        self.store.delete(id)?;
        self.refresh_sessions();
        Ok(())
    }

    /// Check if we're in text input mode (filter or text field).
    pub fn is_text_input_mode(&self) -> bool {
        // Filter mode on sessions screen
        if self.filter_mode {
            return true;
        }
        // Output path field on export screen
        if matches!(self.screen, Screen::Export(_)) {
            return self.export_screen.active_field == super::screens::ExportField::OutputPath;
        }
        false
    }

    /// Handle an action based on current screen.
    pub fn handle_action(&mut self, action: Action) {
        // Handle modal first
        if !matches!(self.modal, ModalState::None) {
            self.handle_modal_action(action);
            return;
        }

        // Handle help toggle - use modal state
        if matches!(action, Action::Help) {
            self.modal = match self.modal {
                ModalState::Help => ModalState::None,
                _ => ModalState::Help,
            };
            return;
        }

        match &self.screen {
            Screen::Sessions => self.handle_sessions_action(action),
            Screen::Detail(_) => self.handle_detail_action(action),
            Screen::Replay(_) => self.handle_replay_action(action),
            Screen::Export(_) => self.handle_export_action(action),
        }
    }

    fn handle_modal_action(&mut self, action: Action) {
        match &self.modal {
            ModalState::DeleteConfirm(id) => {
                let id = id.clone();
                match action {
                    Action::Enter | Action::Char('y') | Action::Char('Y') => {
                        let _ = self.delete_session(&id);
                        self.modal = ModalState::None;
                    }
                    Action::Back | Action::Char('n') | Action::Char('N') => {
                        self.modal = ModalState::None;
                    }
                    _ => {}
                }
            }
            ModalState::Help => {
                if !matches!(action, Action::None) {
                    self.modal = ModalState::None;
                }
            }
            ModalState::None => {}
        }
    }

    fn handle_sessions_action(&mut self, action: Action) {
        let filtered_count = self.filtered_sessions().len();

        match action {
            Action::Quit => self.should_quit = true,
            Action::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            Action::Down => {
                if self.selected + 1 < filtered_count {
                    self.selected += 1;
                }
            }
            Action::First => self.selected = 0,
            Action::Last => {
                if filtered_count > 0 {
                    self.selected = filtered_count - 1;
                }
            }
            Action::Enter => {
                if let Some(session) = self.selected_session() {
                    let id = session.id.to_string();
                    self.filter_mode = false; // Exit filter mode when selecting
                    self.detail_screen = DetailScreen::new(&id);
                    self.screen = Screen::Detail(id);
                }
            }
            Action::Delete => {
                if let Some(session) = self.selected_session() {
                    self.modal = ModalState::DeleteConfirm(session.id.to_string());
                }
            }
            Action::Replay => {
                if let Some(session_info) = self.selected_session() {
                    let id = session_info.id.to_string();
                    let command_count = session_info.command_count;
                    self.replay_screen = ReplayScreen::new(&id, command_count);
                    self.screen = Screen::Replay(id);
                }
            }
            Action::Export => {
                if let Some(session) = self.selected_session() {
                    let id = session.id.to_string();
                    self.export_screen = ExportScreen::new(&id);
                    self.screen = Screen::Export(id);
                }
            }
            Action::Filter => {
                self.filter_mode = true;
            }
            Action::Char(c) if self.filter_mode => {
                self.filter.push(c);
                self.selected = 0;
            }
            Action::Backspace if self.filter_mode => {
                self.filter.pop();
                self.selected = 0;
            }
            Action::Back if self.filter_mode => {
                self.filter_mode = false;
                self.filter.clear();
                self.selected = 0;
            }
            Action::Click(_, row) => {
                // Use the dynamically calculated table start row
                let start_row = self.sessions_screen.table_items_start_row;
                if row >= start_row {
                    let clicked_index = (row - start_row) as usize;
                    if clicked_index < filtered_count {
                        self.selected = clicked_index;
                    }
                }
            }
            Action::Scroll(delta) => {
                if delta < 0 {
                    // Scroll up
                    if self.selected > 0 {
                        self.selected -= 1;
                    }
                } else if delta > 0 {
                    // Scroll down
                    if self.selected + 1 < filtered_count {
                        self.selected += 1;
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_detail_action(&mut self, action: Action) {
        match action {
            Action::Back | Action::Quit => {
                self.screen = Screen::Sessions;
            }
            Action::Up => self.detail_screen.scroll_up(),
            Action::Down => self.detail_screen.scroll_down(),
            Action::Replay => {
                if let Screen::Detail(id) = &self.screen {
                    let id = id.clone();
                    // Get command count from session
                    let command_count = self
                        .load_session(&id)
                        .map(|s| s.commands.len())
                        .unwrap_or(0);
                    self.replay_screen = ReplayScreen::new(&id, command_count);
                    self.screen = Screen::Replay(id);
                }
            }
            Action::Export => {
                if let Screen::Detail(id) = &self.screen {
                    let id = id.clone();
                    self.export_screen = ExportScreen::new(&id);
                    self.screen = Screen::Export(id);
                }
            }
            _ => {}
        }
    }

    fn handle_replay_action(&mut self, action: Action) {
        match action {
            Action::Back | Action::Abort | Action::Quit => {
                self.screen = Screen::Sessions;
            }
            Action::Pause => self.replay_screen.toggle_pause(),
            Action::Skip => self.replay_screen.skip_current(),
            Action::Enter => self.replay_screen.run_next(),
            Action::ScrollUp => self.replay_screen.scroll_output_up(),
            Action::ScrollDown => self.replay_screen.scroll_output_down(),
            _ => {}
        }
    }

    fn handle_export_action(&mut self, action: Action) {
        match action {
            Action::Back => {
                self.screen = Screen::Sessions;
            }
            Action::Up => self.export_screen.prev_format(),
            Action::Down => self.export_screen.next_format(),
            Action::Tab => self.export_screen.next_field(),
            Action::Toggle => self.export_screen.toggle_parameterize(),
            Action::Enter => {
                if self.export_screen.can_confirm() {
                    // Get the session and perform export
                    if let Screen::Export(id) = &self.screen {
                        if let Some(session) = self.load_session(id) {
                            match self.export_screen.do_export(&session) {
                                Ok(()) => {
                                    // Export successful, go back
                                    self.screen = Screen::Sessions;
                                }
                                Err(e) => {
                                    // Store error message
                                    self.export_screen.error_message = Some(e);
                                }
                            }
                        }
                    }
                } else if self.export_screen.output_path.is_empty() {
                    // If path is empty, go to output path field
                    self.export_screen.active_field = super::screens::ExportField::OutputPath;
                } else {
                    // Otherwise, move to next field (or confirm if ready)
                    self.export_screen.next_field();
                }
            }
            Action::Char(c) => self.export_screen.input_char(c),
            Action::Backspace => self.export_screen.backspace(),
            _ => {}
        }
    }
}

/// Setup the terminal for TUI mode.
fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

/// Restore the terminal to normal mode.
fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Run the TUI application.
pub fn run(paths: &Paths) -> Result<()> {
    let mut terminal = setup_terminal().map_err(crate::error::RecError::Io)?;
    let mut app = App::new(paths)?;

    let result = run_app(&mut terminal, &mut app);

    // Restore terminal even if there was an error
    let _ = restore_terminal(&mut terminal);

    result
}

/// Main application loop.
fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    loop {
        // Draw UI
        terminal.draw(|frame| ui::render(frame, app))?;

        // Poll for events
        if let Some(ev) = event::poll_event(Duration::from_millis(100)) {
            let action = event::handle_event(ev, app.is_text_input_mode());
            app.handle_action(action);
        }

        // Check if we should quit
        if app.should_quit {
            break;
        }
    }

    Ok(())
}
