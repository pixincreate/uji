//! Raw terminal input reader: polls crossterm off-thread and forwards events
//! into the loop channel.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::Event as TermEvent;

/// How long the input reader waits between polls (ms).
const INPUT_POLL: Duration = Duration::from_millis(100);

/// Spawn a reader thread that forwards terminal events to `sender`.
///
/// crossterm does not expose its tty fd, so we poll off-thread and wake the
/// loop through the channel. (A direct-fd wakeup is the Unix refinement.)
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
