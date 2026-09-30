use arboard::Clipboard;
use uji_macros::function;

use crate::images::{self, Fitted, ImageError};
use crate::io::{self, Blocked};
use crate::kernel::State;

fn open(slot: &mut Option<Clipboard>) -> Result<&mut Clipboard, arboard::Error> {
    if slot.is_none() {
        *slot = Some(Clipboard::new()?);
    }
    slot.as_mut().ok_or(arboard::Error::ClipboardNotSupported)
}

#[function(clipboard)]
fn get(state: &mut State) -> Result<String, arboard::Error> {
    open(&mut state.clipboard).and_then(Clipboard::get_text)
}

#[function(clipboard)]
fn set(state: &mut State, text: &str) -> Result<(), arboard::Error> {
    open(&mut state.clipboard).and_then(|clipboard| clipboard.set_text(text))
}

fn copied(edge: u32, limit: usize) -> Result<Fitted, ImageError> {
    let raw = match Clipboard::new()?.get_image() {
        Ok(raw) => raw,
        Err(arboard::Error::ContentNotAvailable) => return Err(ImageError::Empty),
        Err(err) => return Err(err.into()),
    };
    let image = images::rgba(raw.width, raw.height, raw.bytes.into_owned())?;
    images::fit_image(image, edge, limit, false)
}

#[function(clipboard)]
async fn image(edge: u32, limit: usize) -> Result<Fitted, Blocked<ImageError>> {
    io::blocking(move || copied(edge, limit)).await
}
