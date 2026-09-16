use std::io::{self, Stdout, Write};

use crossterm::{
    event::{DisableBracketedPaste, EnableBracketedPaste},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::app::App;
use crate::ui;

const ENABLE_MOUSE: &str = "\x1b[?1000h\x1b[?1002h\x1b[?1006h";
const DISABLE_MOUSE: &str = "\x1b[?1006l\x1b[?1002l\x1b[?1000l";

pub type Term = Terminal<CrosstermBackend<Stdout>>;

pub fn open() -> io::Result<Term> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    write!(stdout, "{ENABLE_MOUSE}")?;
    execute!(stdout, EnableBracketedPaste)?;
    stdout.flush()?;
    Terminal::new(CrosstermBackend::new(stdout))
}

pub fn restore(terminal: &mut Term) -> io::Result<()> {
    disable_raw_mode()?;
    write!(terminal.backend_mut(), "{DISABLE_MOUSE}")?;
    execute!(terminal.backend_mut(), DisableBracketedPaste)?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()
}

pub fn resume(terminal: &mut Term) -> io::Result<()> {
    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    write!(terminal.backend_mut(), "{ENABLE_MOUSE}")?;
    execute!(terminal.backend_mut(), EnableBracketedPaste)?;
    terminal.clear()?;
    terminal.hide_cursor()
}

pub fn draw(terminal: &mut Term, app: &App) -> io::Result<()> {
    terminal.draw(|frame| ui::render(frame, app)).map(|_| ())
}
