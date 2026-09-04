use std::io::{self, Stdout};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::{action, storage::session::Session, ui};

/// TUI application state.
pub struct App {
    pub input: String,
    pub cursor: usize,
    pub running: bool,
    pub session: Session,
    pub messages: Vec<String>,
}

impl App {
    pub fn new(session: Session) -> Self {
        Self {
            input: String::new(),
            cursor: 0,
            running: true,
            session,
            messages: Vec::new(),
        }
    }
}

/// Enter raw mode, run the TUI, and restore the terminal on the way out.
pub fn run(session: Session) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(session);
    let result = run_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> io::Result<()> {
    while app.running {
        terminal.draw(|frame| ui::render(frame, app))?;

        if let Event::Key(key) = event::read()? {
            handle_key(key, app);
        }
    }
    Ok(())
}

fn handle_key(key: KeyEvent, app: &mut App) {
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
        KeyCode::Enter => {
            let text = app.input.trim().to_string();
            if !text.is_empty() {
                app.messages.push(format!("> {text}"));
                if let Some(response) = action::run(&text) {
                    app.messages.push(response);
                }
            }
            app.input.clear();
            app.cursor = 0;
        }
        _ => {}
    }
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
