use std::collections::HashMap;
use std::str::FromStr;

use strum::{EnumString, IntoStaticStr, VariantArray};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, IntoStaticStr, VariantArray)]
#[strum(serialize_all = "snake_case")]
pub enum Mode {
    Normal,
    Confirm,
    Select,
    Prompt,
    Suggest,
}

impl Mode {
    pub fn parse(name: &str) -> Option<Self> {
        Self::from_str(name).ok()
    }

    pub fn name(self) -> &'static str {
        self.into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Backspace,
    Delete,
    Tab,
    BackTab,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    F(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    pub key: Key,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Chord {
    pub const fn plain(key: Key) -> Self {
        Self::modified(key, false, false, false)
    }

    pub const fn ctrl(key: Key) -> Self {
        Self::modified(key, true, false, false)
    }

    const fn modified(key: Key, ctrl: bool, alt: bool, shift: bool) -> Self {
        Self {
            key,
            ctrl,
            alt,
            shift,
        }
    }

    pub fn new(key: Key, ctrl: bool, alt: bool, shift: bool) -> Self {
        let key = match key {
            Key::Char(c) => Key::Char(c.to_ascii_lowercase()),
            other => other,
        };
        let shift = match key {
            Key::Char(_) => false,
            Key::BackTab => true,
            _ => shift,
        };
        Self {
            key,
            ctrl,
            alt,
            shift,
        }
    }

    pub fn parse(spec: &str) -> Option<Self> {
        let spec = spec.trim();
        if spec.is_empty() {
            return None;
        }
        if !(spec.starts_with('<') && spec.ends_with('>')) {
            let mut chars = spec.chars();
            let first = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            return Some(Self::new(Key::Char(first), false, false, false));
        }
        let mut rest = &spec[1..spec.len() - 1];
        let mut ctrl = false;
        let mut alt = false;
        let mut shift = false;
        while let Some((prefix, tail)) = rest.split_at_checked(2) {
            match prefix.to_ascii_lowercase().as_str() {
                "c-" => ctrl = true,
                "a-" | "m-" => alt = true,
                "s-" if !tail.eq_ignore_ascii_case("tab") => shift = true,
                _ => break,
            }
            rest = tail;
        }
        let key = parse_key(rest)?;
        Some(Self::new(key, ctrl, alt, shift))
    }
}

fn parse_key(name: &str) -> Option<Key> {
    let lowered = name.to_ascii_lowercase();
    let key = match lowered.as_str() {
        "cr" | "enter" | "return" => Key::Enter,
        "esc" | "escape" => Key::Escape,
        "bs" | "backspace" => Key::Backspace,
        "del" | "delete" => Key::Delete,
        "tab" => Key::Tab,
        "s-tab" | "backtab" => Key::BackTab,
        "left" => Key::Left,
        "right" => Key::Right,
        "up" => Key::Up,
        "down" => Key::Down,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" | "pgup" => Key::PageUp,
        "pagedown" | "pgdn" => Key::PageDown,
        "insert" => Key::Insert,
        "space" => Key::Char(' '),
        "lt" => Key::Char('<'),
        "gt" => Key::Char('>'),
        _ => {
            if let Some(number) = lowered.strip_prefix('f')
                && let Ok(number) = number.parse::<u8>()
                && (1..=12).contains(&number)
            {
                Key::F(number)
            } else {
                let mut chars = name.chars();
                let first = chars.next()?;
                if chars.next().is_some() {
                    return None;
                }
                Key::Char(first)
            }
        }
    };
    Some(key)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binding {
    Action(String),
    Command(String),
    Unbound,
}

const DEFAULTS: &[(Chord, &str)] = &[(Chord::ctrl(Key::Char('c')), "quit")];

#[derive(Debug)]
pub struct Keymap {
    map: HashMap<(Mode, Chord), Binding>,
}

impl Default for Keymap {
    fn default() -> Self {
        let mut keymap = Self {
            map: HashMap::new(),
        };
        keymap.reset();
        keymap
    }
}

impl Keymap {
    pub fn set(&mut self, mode: Mode, chord: Chord, binding: Binding) {
        self.map.insert((mode, chord), binding);
    }

    pub fn get(&self, mode: Mode, chord: Chord) -> Option<&Binding> {
        self.map.get(&(mode, chord))
    }

    pub fn reset(&mut self) {
        self.map.clear();
        for (chord, action) in DEFAULTS {
            for mode in Mode::VARIANTS.iter().copied() {
                self.set(mode, *chord, Binding::Action((*action).to_string()));
            }
        }
    }

    pub fn entries(&self) -> impl Iterator<Item = (&(Mode, Chord), &Binding)> {
        self.map.iter()
    }
}

pub fn describe(chord: Chord) -> String {
    let name = match chord.key {
        Key::Char(' ') => String::from("Space"),
        Key::Char(c) => c.to_string(),
        Key::Enter => String::from("CR"),
        Key::Escape => String::from("Esc"),
        Key::Backspace => String::from("BS"),
        Key::Delete => String::from("Del"),
        Key::Tab => String::from("Tab"),
        Key::BackTab => String::from("S-Tab"),
        Key::Left => String::from("Left"),
        Key::Right => String::from("Right"),
        Key::Up => String::from("Up"),
        Key::Down => String::from("Down"),
        Key::Home => String::from("Home"),
        Key::End => String::from("End"),
        Key::PageUp => String::from("PageUp"),
        Key::PageDown => String::from("PageDown"),
        Key::Insert => String::from("Insert"),
        Key::F(number) => format!("F{number}"),
    };
    let bare =
        matches!(chord.key, Key::Char(c) if c != ' ') && !chord.ctrl && !chord.alt && !chord.shift;
    if bare {
        return name;
    }
    let mut out = String::from("<");
    if chord.ctrl {
        out.push_str("C-");
    }
    if chord.alt {
        out.push_str("A-");
    }
    if chord.shift && !matches!(chord.key, Key::BackTab) {
        out.push_str("S-");
    }
    out.push_str(&name);
    out.push('>');
    out
}
