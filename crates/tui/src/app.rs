use std::io::{self, Stdout};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use uji_core::action;
use uji_core::session::model::{Message, Session, StoredMessage};
use uji_core::session::store::SessionStorage;

use crate::ui;

/// TUI application state.
pub struct App<'a, S: SessionStorage> {
    pub input: String,
    pub cursor: usize,
    pub running: bool,
    pub session: Session,
    pub messages: Vec<StoredMessage>,
    pub storage: &'a S,
}

/// Enter raw mode, run the TUI, and restore the terminal on the way out.
pub fn run<S: SessionStorage>(session: Session, storage: &S) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let messages = storage
        .messages(&session.id)
        .map_err(io::Error::other)?;
    let mut app = App {
        input: String::new(),
        cursor: 0,
        running: true,
        session,
        messages,
        storage,
    };
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
    while app.running {
        terminal.draw(|frame| ui::render(frame, app))?;

        if let Event::Key(key) = event::read()? {
            handle_key(key, app);
        }
    }
    Ok(())
}

fn handle_key<S: SessionStorage>(key: KeyEvent, app: &mut App<'_, S>) {
    match key.code {
        KeyCode::Esc => app.running = false,
        KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => app.running = false,
        KeyCode::Char('q') if key.modifiers == KeyModifiers::NONE => app.running = false,
        KeyCode::Char(c) => {
            app.input.insert(app.cursor, c);
            app.cursor += c.len_utf8();
        }
        KeyCode::Backspace => {
            if app.cursor > 0 {
                let prev = prev_char_boundary(&app.input, app.cursor);
                app.input.remove(prev);
                app.cursor = prev;
            }
        }
        KeyCode::Left => app.cursor = prev_char_boundary(&app.input, app.cursor),
        KeyCode::Right => app.cursor = next_char_boundary(&app.input, app.cursor),
        KeyCode::Enter => submit(app),
        _ => {}
    }
}

/// Submit the current input: persist a user message, dispatch the action,
/// append any response, and clear the input line.
fn submit<S: SessionStorage>(app: &mut App<'_, S>) {
    let text = app.input.trim().to_string();
    if !text.is_empty() {
        let user = Message::User { text: text.clone() };
        if let Ok(stored) = app.storage.append_message(&app.session.id, user) {
            app.messages.push(stored);
        }
        if let Some(response) = action::run(&text) {
            let assistant = Message::Assistant { text: response };
            if let Ok(stored) = app.storage.append_message(&app.session.id, assistant) {
                app.messages.push(stored);
            }
        }
    }
    app.input.clear();
    app.cursor = 0;
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
