use arboard::Clipboard;
use mlua::Lua;
use uji_macros::function;

use crate::images::{self, Fitted, ImageError};
use crate::io;
use crate::kernel::State;

fn open(slot: &mut Option<Clipboard>) -> Result<&mut Clipboard, arboard::Error> {
    if slot.is_none() {
        *slot = Some(Clipboard::new()?);
    }
    slot.as_mut().ok_or(arboard::Error::ClipboardNotSupported)
}

#[function(clipboard)]
fn get(lua: &Lua) -> mlua::Result<Result<String, arboard::Error>> {
    Ok(open(&mut State::of_mut(lua)?.clipboard).and_then(Clipboard::get_text))
}

#[function(clipboard)]
fn set(lua: &Lua, text: String) -> mlua::Result<Result<bool, arboard::Error>> {
    Ok(open(&mut State::of_mut(lua)?.clipboard)
        .and_then(|clipboard| clipboard.set_text(text))
        .map(|()| true))
}

#[function(clipboard)]
async fn image(lua: Lua, edge: u32, limit: usize) -> mlua::Result<Result<Fitted, ImageError>> {
    let copied = open(&mut State::of_mut(&lua)?.clipboard).and_then(Clipboard::get_image);
    let raw = match copied {
        Ok(raw) => raw,
        Err(arboard::Error::ContentNotAvailable) => return Ok(Err(ImageError::Empty)),
        Err(err) => return Ok(Err(err.into())),
    };
    let (width, height, pixels) = (raw.width, raw.height, raw.bytes.into_owned());
    io::blocking(&lua, move || {
        images::rgba(width, height, pixels)
            .and_then(|image| images::fit_image(image, edge, limit, false))
    })
    .await
}
