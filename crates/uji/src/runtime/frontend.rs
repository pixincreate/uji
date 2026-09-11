use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use calloop::channel::Sender;
use crossterm::event::Event as TermEvent;
use uji_tui::app::{self, App};

pub trait Frontend {
    fn start(&mut self, events: Sender<TermEvent>, paused: Arc<AtomicBool>) -> io::Result<()>;
    fn draw(&mut self, app: &App) -> io::Result<()>;
    fn suspend(&mut self) -> io::Result<()>;
    fn resume(&mut self) -> io::Result<()>;
    fn stop(&mut self) -> io::Result<()>;
}

#[derive(Default)]
pub struct Terminal {
    term: Option<app::Term>,
    reader: Option<Arc<AtomicBool>>,
}

impl Terminal {
    pub fn new() -> Self {
        Self::default()
    }

    fn term(&mut self) -> io::Result<&mut app::Term> {
        self.term
            .as_mut()
            .ok_or_else(|| io::Error::other("terminal is not attached"))
    }
}

impl Frontend for Terminal {
    fn start(&mut self, events: Sender<TermEvent>, paused: Arc<AtomicBool>) -> io::Result<()> {
        self.term = Some(app::setup()?);
        let running = Arc::new(AtomicBool::new(true));
        self.reader = Some(Arc::clone(&running));
        super::input::spawn(events, running, paused);
        Ok(())
    }

    fn draw(&mut self, app: &App) -> io::Result<()> {
        app::draw(self.term()?, app)
    }

    fn suspend(&mut self) -> io::Result<()> {
        app::restore(self.term()?)
    }

    fn resume(&mut self) -> io::Result<()> {
        app::resume(self.term()?)
    }

    fn stop(&mut self) -> io::Result<()> {
        if let Some(running) = self.reader.take() {
            running.store(false, Ordering::Relaxed);
        }
        match self.term.as_mut() {
            Some(term) => app::restore(term),
            None => Ok(()),
        }
    }
}
