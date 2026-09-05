//! TUI application data and terminal setup.
//!
//! The loop itself lives in `libuji`'s runtime (nvim keeps its loop in
//! core too); this module owns the app state, decodes keys into
//! [`KeyAction`]s, and provides terminal setup/teardown plus drawing.

use std::cell::RefCell;
use std::io::{self, Stdout};
use std::rc::Rc;

use crossterm::{
    event::{KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use uji_core::session::model::{Session, StoredMessage};

use crate::state::UiState;
use crate::ui;

/// What the runtime should do after a key press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    /// Key consumed; nothing further needed.
    None,
    /// The loop should exit.
    Quit,
    /// The input line was submitted.
    Submit(String),
}

/// TUI application state for one session run.
///
/// Fields are private; the renderer goes through the accessors and all
/// mutation flows through [`App::handle_key`].
pub struct App {
    session: Session,
    messages: Vec<StoredMessage>,
    state: Rc<RefCell<UiState>>,
    input: String,
    cursor: usize,
    running: bool,
}

impl App {
    /// Create app state for `session` with its persisted history and the
    /// live UI state.
    pub fn new(
        session: Session,
        messages: Vec<StoredMessage>,
        state: Rc<RefCell<UiState>>,
    ) -> Self {
        Self {
            session,
            messages,
            state,
            input: String::new(),
            cursor: 0,
            running: true,
        }
    }

    /// The session being run.
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// The message history rendered in the messages buffer.
    pub fn messages(&self) -> &[StoredMessage] {
        &self.messages
    }

    /// Append a message to the rendered history.
    pub fn push_message(&mut self, message: StoredMessage) {
        self.messages.push(message);
    }

    /// The live UI state, re-projected every frame.
    pub fn state(&self) -> &Rc<RefCell<UiState>> {
        &self.state
    }

    /// The current input line.
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Byte offset of the cursor within [`App::input`].
    pub fn cursor_offset(&self) -> usize {
        self.cursor
    }

    /// Whether the event loop should keep running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Apply a key press to the input line and report what the runtime
    /// should do next.
    pub fn handle_key(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Esc => self.quit(),
            KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => self.quit(),
            KeyCode::Char('q') if key.modifiers == KeyModifiers::NONE => self.quit(),
            KeyCode::Char(c) => {
                self.input.insert(self.cursor, c);
                self.cursor += c.len_utf8();
                KeyAction::None
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    let prev = prev_char_boundary(&self.input, self.cursor);
                    self.input.remove(prev);
                    self.cursor = prev;
                }
                KeyAction::None
            }
            KeyCode::Left => {
                self.cursor = prev_char_boundary(&self.input, self.cursor);
                KeyAction::None
            }
            KeyCode::Right => {
                self.cursor = next_char_boundary(&self.input, self.cursor);
                KeyAction::None
            }
            KeyCode::Enter => self.take_submit(),
            _ => KeyAction::None,
        }
    }

    fn quit(&mut self) -> KeyAction {
        self.running = false;
        KeyAction::Quit
    }

    fn take_submit(&mut self) -> KeyAction {
        let text = self.input.trim().to_string();
        self.input.clear();
        self.cursor = 0;
        if text.is_empty() {
            KeyAction::None
        } else {
            KeyAction::Submit(text)
        }
    }
}

/// Terminal type used by the loop.
pub type Term = Terminal<CrosstermBackend<Stdout>>;

/// Enter raw mode + the alternate screen and build a terminal.
pub fn setup() -> io::Result<Term> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

/// Leave the alternate screen and restore the terminal.
pub fn restore(terminal: &mut Term) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()
}

/// Project the app state onto the terminal (nvim's `ui_flush`).
pub fn draw(terminal: &mut Term, app: &App) -> io::Result<()> {
    terminal.draw(|frame| ui::render(frame, app)).map(|_| ())
}

/// Move `index` back to the previous char boundary (or 0).
fn prev_char_boundary(s: &str, index: usize) -> usize {
    if index == 0 {
        return 0;
    }
    let mut i = index - 1;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Move `index` forward to the next char boundary (or `s.len()`).
fn next_char_boundary(s: &str, index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    let mut i = index + 1;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;
    use uji_core::session::id::SessionId;
    use uji_core::session::model::Time;

    fn app() -> App {
        let session = Session {
            id: SessionId::new(),
            parent_id: None,
            title: "test".into(),
            directory: String::new(),
            time: Time {
                created: 0,
                updated: 0,
            },
        };
        App::new(session, Vec::new(), Rc::new(RefCell::new(UiState::new())))
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn typing_accumulates_and_enter_submits() {
        let mut app = app();
        assert_eq!(app.handle_key(key(KeyCode::Char('h'))), KeyAction::None);
        assert_eq!(app.handle_key(key(KeyCode::Char('i'))), KeyAction::None);
        assert_eq!(app.input(), "hi");

        assert_eq!(
            app.handle_key(key(KeyCode::Enter)),
            KeyAction::Submit("hi".into())
        );
        assert_eq!(app.input(), "");
    }

    #[test]
    fn enter_on_empty_input_does_nothing() {
        let mut app = app();
        assert_eq!(app.handle_key(key(KeyCode::Enter)), KeyAction::None);
    }

    #[test]
    fn quit_keys_stop_the_app() {
        let mut app = app();
        assert_eq!(app.handle_key(key(KeyCode::Esc)), KeyAction::Quit);
        assert!(!app.is_running());
    }

    #[test]
    fn backspace_and_arrows_edit_safely() {
        let mut app = app();
        for c in "aéz".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Left));
        assert_eq!(app.cursor_offset(), 3);
        app.handle_key(key(KeyCode::Backspace));
        assert_eq!(app.input(), "az");
        app.handle_key(key(KeyCode::Right));
        assert_eq!(app.cursor_offset(), 2);
    }
}
