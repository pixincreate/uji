use base64::Engine;
use base64::engine::GeneralPurpose;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use mlua::{BString, ExternalResult, Lua, LuaSerdeExt, Value};
use rand::RngCore;
use sha2::{Digest, Sha256};
use uji_macros::{function, options};
use unicode_width::UnicodeWidthStr;

use crate::io;

#[options]
struct Base64Options {
    #[serde(default)]
    url: bool,
    #[serde(default = "padded")]
    pad: bool,
}

fn padded() -> bool {
    true
}

impl Base64Options {
    fn engine(&self) -> &GeneralPurpose {
        match (self.url, self.pad) {
            (false, true) => &STANDARD,
            (false, false) => &STANDARD_NO_PAD,
            (true, true) => &URL_SAFE,
            (true, false) => &URL_SAFE_NO_PAD,
        }
    }
}

#[function(base64)]
fn encode(data: &[u8], opts: &Base64Options) -> String {
    opts.engine().encode(data)
}

#[function(base64, raise)]
fn decode(text: &[u8], opts: &Base64Options) -> Result<BString, base64::DecodeError> {
    opts.engine().decode(text).map(BString::from)
}

#[function(toml)]
fn decode(lua: &Lua, text: &str) -> mlua::Result<Value> {
    io::to_lua(lua, &text.parse::<toml::Table>().into_lua_err()?)
}

#[function(toml)]
fn encode(lua: &Lua, value: Value) -> mlua::Result<String> {
    toml::to_string(&lua.from_value::<toml::Table>(value)?).into_lua_err()
}

#[function]
fn sha256(data: &[u8]) -> BString {
    Sha256::digest(data).to_vec().into()
}

#[function]
fn random(count: usize) -> BString {
    let mut bytes = vec![0; count];
    rand::rng().fill_bytes(&mut bytes);
    bytes.into()
}

#[function]
fn lossy(text: &str) -> String {
    text.to_owned()
}

#[function]
fn width(text: &str) -> usize {
    text.width()
}
