use arboard::Clipboard;
use uji_native::{Json, native};

use crate::context;
use crate::images::{self, Fitted, ImageError};
use crate::io::{self, Blocked};

fn using<T>(
    run: impl FnOnce(&mut Clipboard) -> Result<T, arboard::Error>,
) -> Result<T, arboard::Error> {
    context::with(|context| {
        if context.clipboard.is_none() {
            context.clipboard = Some(Clipboard::new()?);
        }
        context
            .clipboard
            .as_mut()
            .ok_or(arboard::Error::ClipboardNotSupported)
            .and_then(run)
    })
    .unwrap_or(Err(arboard::Error::ClipboardNotSupported))
}

#[native(clipboard)]
fn get() -> Result<String, arboard::Error> {
    using(Clipboard::get_text)
}

#[native(clipboard)]
fn set(text: &str) -> Result<(), arboard::Error> {
    using(|clipboard| clipboard.set_text(text))
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

#[native(clipboard)]
async fn image(edge: u32, limit: usize) -> Result<Json<Fitted>, Blocked<ImageError>> {
    io::blocking(move || copied(edge, limit)).await.map(Json)
}
