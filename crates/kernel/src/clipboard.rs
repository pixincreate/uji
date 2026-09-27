use arboard::Clipboard;
use mlua::Lua;
use uji_macros::function;

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
