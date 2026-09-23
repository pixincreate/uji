use std::io::{self, Stdout, Write};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::{
    event::{
        DisableBracketedPaste, EnableBracketedPaste, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
        supports_keyboard_enhancement,
    },
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::app::App;
use crate::render;

const ENABLE_MOUSE: &str = "\x1b[?1000h\x1b[?1002h\x1b[?1006h";
const DISABLE_MOUSE: &str = "\x1b[?1006l\x1b[?1002l\x1b[?1000l";

pub type Term = Terminal<CrosstermBackend<Stdout>>;

/// Report keys unambiguously, and say which character a shifted key produces.
///
/// Without this a bare Escape is indistinguishable from the start of any escape
/// sequence, and `ctrl+i`, `ctrl+m` and `ctrl+[` are indistinguishable from Tab,
/// Enter and Escape, so those chords cannot be bound at all.
const ENHANCEMENTS: KeyboardEnhancementFlags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
    .union(KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS);

/// Whether the flags were pushed, so they are popped exactly as often.
static ENHANCED: AtomicBool = AtomicBool::new(false);

static GUARDED: Once = Once::new();

fn push_enhancements(out: &mut impl Write) -> io::Result<()> {
    if !supports_keyboard_enhancement().unwrap_or(false) {
        return Ok(());
    }
    execute!(out, PushKeyboardEnhancementFlags(ENHANCEMENTS))?;
    ENHANCED.store(true, Ordering::Relaxed);
    Ok(())
}

fn pop_enhancements(out: &mut impl Write) -> io::Result<()> {
    if !ENHANCED.swap(false, Ordering::Relaxed) {
        return Ok(());
    }
    execute!(out, PopKeyboardEnhancementFlags)
}

/// Put the terminal back if the process dies unexpectedly.
///
/// Without this a panic leaves raw mode, the alternate screen and the keyboard
/// flags in place, and the shell that comes back is unusable.
fn guard_against_panic() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let mut out = io::stdout();
        let _ = pop_enhancements(&mut out);
        let _ = execute!(out, DisableBracketedPaste, LeaveAlternateScreen);
        let _ = write!(out, "{DISABLE_MOUSE}");
        let _ = disable_raw_mode();
        let _ = out.flush();
        previous(info);
    }));
}

pub fn open() -> io::Result<Term> {
    GUARDED.call_once(guard_against_panic);
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    write!(stdout, "{ENABLE_MOUSE}")?;
    execute!(stdout, EnableBracketedPaste)?;
    push_enhancements(&mut stdout)?;
    stdout.flush()?;
    Terminal::new(CrosstermBackend::new(stdout))
}

pub fn restore(terminal: &mut Term) -> io::Result<()> {
    disable_raw_mode()?;
    pop_enhancements(terminal.backend_mut())?;
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
    push_enhancements(terminal.backend_mut())?;
    terminal.clear()?;
    terminal.hide_cursor()
}

pub fn draw(terminal: &mut Term, app: &App) -> io::Result<()> {
    terminal
        .draw(|frame| render::render(frame, app))
        .map(|_| ())
}
