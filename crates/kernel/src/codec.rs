use base64::Engine;
use base64::engine::GeneralPurpose;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uji_native::{Json, native};
use unicode_width::UnicodeWidthStr;

#[derive(Deserialize)]
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
    fn engine(&self) -> &'static GeneralPurpose {
        match (self.url, self.pad) {
            (false, true) => &STANDARD,
            (false, false) => &STANDARD_NO_PAD,
            (true, true) => &URL_SAFE,
            (true, false) => &URL_SAFE_NO_PAD,
        }
    }
}

#[native(base64)]
fn encode(data: &[u8], opts: Json<Base64Options>) -> String {
    let Json(opts) = opts;
    opts.engine().encode(data)
}

#[native(base64, raise)]
fn decode(text: &[u8], opts: Json<Base64Options>) -> Result<Vec<u8>, base64::DecodeError> {
    let Json(opts) = opts;
    opts.engine().decode(text)
}

#[native(toml, raise)]
fn decode(text: &str) -> Result<Json<toml::Table>, toml::de::Error> {
    text.parse::<toml::Table>().map(Json)
}

#[native(toml, raise)]
fn encode(value: Json<toml::Table>) -> Result<String, toml::ser::Error> {
    let Json(table) = value;
    toml::to_string(&table)
}

#[native]
fn sha256(data: &[u8]) -> Vec<u8> {
    Sha256::digest(data).to_vec()
}

#[native]
fn random(count: usize) -> Vec<u8> {
    let mut bytes = vec![0; count];
    rand::rng().fill_bytes(&mut bytes);
    bytes
}

#[native]
fn lossy(data: &[u8]) -> String {
    String::from_utf8_lossy(data).into_owned()
}

#[native]
fn width(text: &str) -> usize {
    text.width()
}
