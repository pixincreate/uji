use std::cell::RefCell;
use std::io::Write;

use base64::Engine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Copied {
    Native,
    Osc52,
}

thread_local! {
    static NATIVE: RefCell<Option<arboard::Clipboard>> = const { RefCell::new(None) };
}

pub fn write(text: &str) -> Copied {
    if native(text) {
        return Copied::Native;
    }
    osc52(text);
    Copied::Osc52
}

fn native(text: &str) -> bool {
    NATIVE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = arboard::Clipboard::new().ok();
        }
        slot.as_mut()
            .is_some_and(|clipboard| clipboard.set_text(text).is_ok())
    })
}

fn osc52(text: &str) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    let mut stdout = std::io::stdout();
    let _ = write!(stdout, "\x1b]52;c;{encoded}\x07");
    let _ = stdout.flush();
}
