mod input;
mod real;
mod screen;
mod virtual_screen;

use std::sync::Arc;

use crossterm::event::Event;
use tokio::sync::{mpsc, watch};
use uji_native::{Held, native};

use crate::context;

pub enum Terminal {
    Real,
    Virtual(VirtualTerminal),
}

pub(crate) enum Tty {
    Fresh(Terminal),
    Opened(screen::Screen, Arc<input::Input>),
}

pub struct VirtualTerminal {
    events: mpsc::UnboundedReceiver<Event>,
    frames: watch::Sender<Vec<String>>,
    size: watch::Receiver<(u16, u16)>,
}

pub struct VirtualHandle {
    events: mpsc::UnboundedSender<Event>,
    frames: watch::Receiver<Vec<String>>,
    size: watch::Sender<(u16, u16)>,
}

impl VirtualHandle {
    pub fn send(&self, event: Event) -> bool {
        self.events.send(event).is_ok()
    }

    pub fn resize(&self, width: u16, height: u16) -> bool {
        self.size.send((width, height)).is_ok() && self.send(Event::Resize(width, height))
    }

    pub fn frame(&self) -> Vec<String> {
        self.frames.borrow().clone()
    }

    pub fn frames(&self) -> watch::Receiver<Vec<String>> {
        self.frames.clone()
    }
}

pub fn virtual_terminal(width: u16, height: u16) -> (VirtualTerminal, VirtualHandle) {
    let (sender, events) = mpsc::unbounded_channel();
    let (published, frames) = watch::channel(Vec::new());
    let (resized, size) = watch::channel((width, height));
    (
        VirtualTerminal {
            events,
            frames: published,
            size,
        },
        VirtualHandle {
            events: sender,
            frames,
            size: resized,
        },
    )
}

pub(crate) fn restore() {
    real::restore();
}

fn opened(terminal: Option<Tty>) -> std::io::Result<(screen::Screen, Arc<input::Input>)> {
    match terminal {
        Some(Tty::Fresh(Terminal::Real)) => real::open(),
        Some(Tty::Fresh(Terminal::Virtual(terminal))) => Ok(virtual_screen::open(terminal)),
        Some(Tty::Opened(screen, input)) => Ok((screen, input)),
        None => Err(std::io::Error::other("this uji has no terminal")),
    }
}

#[native(tty, raise)]
fn open() -> std::io::Result<Held<Arc<input::Input>>> {
    context::with(|context| {
        let (screen, input) = opened(context.terminal.take())?;
        context.terminal = Some(Tty::Opened(screen, Arc::clone(&input)));
        Ok(Held::new(input))
    })
    .unwrap_or_else(|| Err(std::io::Error::other("the kernel is not running")))
}
