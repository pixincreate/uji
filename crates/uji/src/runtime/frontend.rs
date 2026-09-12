use std::io;

use calloop::channel::Sender;
use crossterm::event::Event as TermEvent;
use uji_tui::app::App;
use uji_tui::terminal;

use super::input::Reader;

pub trait Frontend {
    fn start(&mut self, events: Sender<TermEvent>) -> io::Result<()>;
    fn draw(&mut self, app: &App) -> io::Result<()>;
    fn suspend(&mut self) -> io::Result<()>;
    fn resume(&mut self) -> io::Result<()>;
    fn stop(&mut self) -> io::Result<()>;
}

#[derive(Default)]
pub struct Terminal {
    term: Option<terminal::Term>,
    reader: Option<Reader>,
}

impl Terminal {
    pub fn new() -> Self {
        Self::default()
    }

    fn term(&mut self) -> io::Result<&mut terminal::Term> {
        self.term
            .as_mut()
            .ok_or_else(|| io::Error::other("terminal is not attached"))
    }
}

impl Frontend for Terminal {
    fn start(&mut self, events: Sender<TermEvent>) -> io::Result<()> {
        self.term = Some(terminal::open()?);
        self.reader = Some(Reader::spawn(events));
        Ok(())
    }

    fn draw(&mut self, app: &App) -> io::Result<()> {
        terminal::draw(self.term()?, app)
    }

    fn suspend(&mut self) -> io::Result<()> {
        if let Some(reader) = &self.reader {
            reader.pause();
        }
        terminal::restore(self.term()?)
    }

    fn resume(&mut self) -> io::Result<()> {
        let resumed = terminal::resume(self.term()?);
        if let Some(reader) = &self.reader {
            reader.resume();
        }
        resumed
    }

    fn stop(&mut self) -> io::Result<()> {
        if let Some(reader) = self.reader.take() {
            reader.stop();
        }
        match self.term.as_mut() {
            Some(term) => terminal::restore(term),
            None => Ok(()),
        }
    }
}
