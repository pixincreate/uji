//! TUI application loop: terminal setup, key handling, message submission.

use std::io::{self, Stdout};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use uji_core::action;
use uji_core::session::model::{Message, Session, StoredMessage};
use uji_core::session::store::SessionStorage;

use crate::model::UiModel;
use crate::ui;

/// TUI application state for one session run.
///
/// Fields are private; the renderer goes through the accessors and all
/// mutation flows through the key handlers.
pub struct App<'a, S: SessionStorage> {
    session: Session,
    messages: Vec<StoredMessage>,
    storage: &'a mut S,
    model: UiModel,
    input: String,
    cursor: usize,
    running: bool,
}

impl<'a, S: SessionStorage> App<'a, S> {
    fn new(
        session: Session,
        storage: &'a mut S,
        messages: Vec<StoredMessage>,
        model: UiModel,
    ) -> Self {
        Self {
            session,
            messages,
            storage,
            model,
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

    /// The active UI model.
    pub fn model(&self) -> &UiModel {
        &self.model
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

    fn quit(&mut self) {
        self.running = false;
    }

    fn insert_char(&mut self, ch: char) {
        self.input.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            let prev = prev_char_boundary(&self.input, self.cursor);
            self.input.remove(prev);
            self.cursor = prev;
        }
    }

    fn cursor_left(&mut self) {
        self.cursor = prev_char_boundary(&self.input, self.cursor);
    }

    fn cursor_right(&mut self) {
        self.cursor = next_char_boundary(&self.input, self.cursor);
    }

    /// Submit the current input: persist a user message, dispatch the
    /// action, append any response, and clear the input line.
    fn submit(&mut self) {
        let text = self.input.trim().to_string();
        if !text.is_empty() {
            let user = Message::User { text: text.clone() };
            if let Ok(stored) = self.storage.append_message(&self.session.id, user) {
                self.messages.push(stored);
            }
            if let Some(response) = action::run(&text) {
                let assistant = Message::Assistant { text: response };
                if let Ok(stored) = self.storage.append_message(&self.session.id, assistant) {
                    self.messages.push(stored);
                }
            }
        }
        self.input.clear();
        self.cursor = 0;
    }

    /// Dispatch a terminal key event.
    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.quit(),
            KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => self.quit(),
            KeyCode::Char('q') if key.modifiers == KeyModifiers::NONE => self.quit(),
            KeyCode::Char(c) => self.insert_char(c),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Left => self.cursor_left(),
            KeyCode::Right => self.cursor_right(),
            KeyCode::Enter => self.submit(),
            _ => {}
        }
    }
}

/// Enter raw mode, run the TUI, and restore the terminal on the way out.
pub fn run<S: SessionStorage>(session: Session, storage: &mut S, model: UiModel) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let messages = storage.messages(&session.id).map_err(io::Error::other)?;
    let mut app = App::new(session, storage, messages, model);
    let result = run_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_loop<S: SessionStorage>(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App<'_, S>,
) -> io::Result<()> {
    while app.is_running() {
        terminal.draw(|frame| ui::render(frame, app))?;

        if let Event::Key(key) = event::read()? {
            app.handle_key(key);
        }
    }
    Ok(())
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

    #[test]
    fn char_boundary_helpers_walk_grapheme_safe_offsets() {
        let s = "aéz";
        // byte offsets: a=0, é=1..3, z=3
        assert_eq!(prev_char_boundary(s, 3), 1);
        assert_eq!(prev_char_boundary(s, 1), 0);
        assert_eq!(prev_char_boundary(s, 0), 0);
        assert_eq!(next_char_boundary(s, 0), 1);
        assert_eq!(next_char_boundary(s, 1), 3);
        assert_eq!(next_char_boundary(s, 3), 4);
        assert_eq!(next_char_boundary(s, 4), 4);
    }
}
