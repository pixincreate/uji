use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{
    Event as TermEvent, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::app::selection::Point;
use crate::keymap::{Chord, Key};

const POLL: Duration = Duration::from_millis(100);

/// Something the user did, in uji's own terms.
///
/// The terminal library stops here: everything above this layer works in
/// [`Chord`]s and [`Point`]s, not in whatever crossterm happens to call them.
pub enum Input {
    Key(Chord),
    Paste(String),
    Mouse { point: Point, kind: MouseKind },
    Resize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MouseKind {
    Press,
    Drag,
    Release,
    ScrollUp,
    ScrollDown,
}

fn chord_of(key: KeyEvent) -> Option<Chord> {
    let mapped = match key.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Escape,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::Tab => Key::Tab,
        KeyCode::BackTab => Key::BackTab,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Insert => Key::Insert,
        KeyCode::F(number) => Key::F(number),
        _ => return None,
    };
    Some(Chord::new(
        mapped,
        key.modifiers.contains(KeyModifiers::CONTROL),
        key.modifiers.contains(KeyModifiers::ALT),
        key.modifiers.contains(KeyModifiers::SHIFT),
    ))
}

fn mouse_of(mouse: MouseEvent) -> Option<Input> {
    let kind = match mouse.kind {
        MouseEventKind::ScrollUp => MouseKind::ScrollUp,
        MouseEventKind::ScrollDown => MouseKind::ScrollDown,
        MouseEventKind::Down(MouseButton::Left) => MouseKind::Press,
        MouseEventKind::Drag(MouseButton::Left) => MouseKind::Drag,
        MouseEventKind::Up(MouseButton::Left) => MouseKind::Release,
        _ => return None,
    };
    Some(Input::Mouse {
        point: Point::new(mouse.column, mouse.row),
        kind,
    })
}

fn translate(event: TermEvent) -> Option<Input> {
    match event {
        TermEvent::Key(key) => chord_of(key).map(Input::Key),
        TermEvent::Paste(text) => Some(Input::Paste(text)),
        TermEvent::Mouse(mouse) => mouse_of(mouse),
        TermEvent::Resize(..) => Some(Input::Resize),
        _ => None,
    }
}

/// Reads the terminal on its own thread and posts [`Input`] to the loop.
pub struct Reader {
    running: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
}

impl Reader {
    pub fn spawn(sender: calloop::channel::Sender<Input>) -> Self {
        let reader = Self {
            running: Arc::new(AtomicBool::new(true)),
            paused: Arc::new(AtomicBool::new(false)),
        };
        let running = Arc::clone(&reader.running);
        let paused = Arc::clone(&reader.paused);
        std::thread::spawn(move || {
            while running.load(Ordering::Relaxed) {
                if paused.load(Ordering::Relaxed) {
                    std::thread::sleep(POLL);
                    continue;
                }
                if crossterm::event::poll(POLL).unwrap_or(false) {
                    if paused.load(Ordering::Relaxed) {
                        continue;
                    }
                    match crossterm::event::read() {
                        Ok(event) => {
                            if let Some(input) = translate(event)
                                && sender.send(input).is_err()
                            {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        });
        reader
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::Relaxed);
        std::thread::sleep(POLL.saturating_add(POLL / 5));
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}
