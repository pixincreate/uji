use std::ffi::c_void;
use std::fmt;

use serde::de::{DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use uji_native::abi::{self, Out};
use uji_native::{CStruct, native};

const NULL: u8 = 0;
const FALSE: u8 = 1;
const TRUE: u8 = 2;
const NUMBER: u8 = 3;
const STRING: u8 = 4;
const ARRAY: u8 = 5;
const OBJECT: u8 = 6;

#[repr(C)]
#[derive(CStruct, Clone, Copy, Default)]
pub(crate) struct Token {
    pub(crate) kind: u8,
    pub(crate) count: usize,
    pub(crate) offset: usize,
    pub(crate) length: usize,
    pub(crate) number: f64,
}

#[repr(C)]
#[derive(CStruct)]
pub(crate) struct Tokens {
    pub(crate) items: *const Token,
    pub(crate) count: usize,
    pub(crate) strings: *const u8,
}

struct Tape {
    tokens: Vec<Token>,
    strings: Vec<u8>,
}

impl Tape {
    fn push(&mut self, kind: u8) -> usize {
        self.tokens.push(Token {
            kind,
            ..Token::default()
        });
        self.tokens.len() - 1
    }

    fn number(&mut self, number: f64) {
        self.tokens.push(Token {
            kind: NUMBER,
            number,
            ..Token::default()
        });
    }

    fn string(&mut self, text: &str) {
        let offset = self.strings.len();
        self.strings.extend_from_slice(text.as_bytes());
        self.tokens.push(Token {
            kind: STRING,
            offset,
            length: text.len(),
            ..Token::default()
        });
    }
}

struct Seed<'a>(&'a mut Tape);

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Seed<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_unit<E>(self) -> Result<(), E> {
        self.0.push(NULL);
        Ok(())
    }

    fn visit_bool<E>(self, value: bool) -> Result<(), E> {
        self.0.push(if value { TRUE } else { FALSE });
        Ok(())
    }

    fn visit_i64<E>(self, value: i64) -> Result<(), E> {
        self.0
            .number(serde_json::Number::from(value).as_f64().unwrap_or_default());
        Ok(())
    }

    fn visit_u64<E>(self, value: u64) -> Result<(), E> {
        self.0
            .number(serde_json::Number::from(value).as_f64().unwrap_or_default());
        Ok(())
    }

    fn visit_f64<E>(self, value: f64) -> Result<(), E> {
        self.0.number(value);
        Ok(())
    }

    fn visit_str<E>(self, value: &str) -> Result<(), E> {
        self.0.string(value);
        Ok(())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut items: A) -> Result<(), A::Error> {
        let tape = self.0;
        let at = tape.push(ARRAY);
        let mut count = 0;
        while items.next_element_seed(Seed(&mut *tape))?.is_some() {
            count += 1;
        }
        tape.tokens[at].count = count;
        Ok(())
    }

    fn visit_map<A: MapAccess<'de>>(self, mut entries: A) -> Result<(), A::Error> {
        let tape = self.0;
        let at = tape.push(OBJECT);
        let mut count = 0;
        while entries.next_key_seed(Seed(&mut *tape))?.is_some() {
            entries.next_value_seed(Seed(&mut *tape))?;
            count += 1;
        }
        tape.tokens[at].count = count;
        Ok(())
    }
}

#[native(json)]
fn tokens(data: &[u8], out: Out<Tokens>) -> *mut c_void {
    let mut tape = Tape {
        tokens: Vec::new(),
        strings: Vec::with_capacity(data.len()),
    };
    let mut deserializer = serde_json::Deserializer::from_slice(data);
    let parsed = Seed(&mut tape)
        .deserialize(&mut deserializer)
        .and_then(|()| deserializer.end());
    if let Err(err) = parsed {
        abi::fail(&err);
        return std::ptr::null_mut();
    }
    out.write(Tokens {
        items: tape.tokens.as_ptr(),
        count: tape.tokens.len(),
        strings: tape.strings.as_ptr(),
    });
    abi::handle(Box::new(tape))
}
