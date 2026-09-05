use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::Event as TermEvent;

const INPUT_POLL: Duration = Duration::from_millis(100);

pub(crate) fn spawn(sender: calloop::channel::Sender<TermEvent>, running: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        while running.load(Ordering::Relaxed) {
            if crossterm::event::poll(INPUT_POLL).unwrap_or(false) {
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
}
