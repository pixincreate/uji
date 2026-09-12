use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::Event as TermEvent;

const INPUT_POLL: Duration = Duration::from_millis(100);

pub(crate) struct Reader {
    running: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
}

impl Reader {
    pub(crate) fn spawn(sender: calloop::channel::Sender<TermEvent>) -> Self {
        let reader = Self {
            running: Arc::new(AtomicBool::new(true)),
            paused: Arc::new(AtomicBool::new(false)),
        };
        let running = Arc::clone(&reader.running);
        let paused = Arc::clone(&reader.paused);
        std::thread::spawn(move || {
            while running.load(Ordering::Relaxed) {
                if paused.load(Ordering::Relaxed) {
                    std::thread::sleep(INPUT_POLL);
                    continue;
                }
                if crossterm::event::poll(INPUT_POLL).unwrap_or(false) {
                    if paused.load(Ordering::Relaxed) {
                        continue;
                    }
                    match crossterm::event::read() {
                        Ok(event) => {
                            if sender.send(event).is_err() {
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

    pub(crate) fn pause(&self) {
        self.paused.store(true, Ordering::Relaxed);
        std::thread::sleep(INPUT_POLL.saturating_add(INPUT_POLL / 5));
    }

    pub(crate) fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
    }

    pub(crate) fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}
