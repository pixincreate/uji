//! Idiomatic conversions for the UI model: `FromStr`/`Display` for the
//! string-keyed enums and `From<u16>` for fixed sizes.
//!
//! Kept in one file so the string forms are auditable in a single place. The
//! Lua-value conversions live in `libuji::lua::convert` (they need `mlua`).

use std::fmt;
use std::str::FromStr;

use crate::model::{Border, BufferKind, Size, Split};

/// Error from parsing a model value from a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

impl FromStr for BufferKind {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "messages" => Ok(Self::Messages),
            "input" => Ok(Self::Input),
            other => Err(ParseError(format!(
                "unknown buffer kind: {other} (expected \"messages\" or \"input\")"
            ))),
        }
    }
}

impl fmt::Display for BufferKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Messages => "messages",
            Self::Input => "input",
        })
    }
}

impl FromStr for Split {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "top" => Ok(Self::Top),
            "bottom" => Ok(Self::Bottom),
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            other => Err(ParseError(format!("unknown split: {other}"))),
        }
    }
}

impl fmt::Display for Split {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::Right => "right",
        })
    }
}

impl FromStr for Border {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "plain" => Ok(Self::Plain),
            "rounded" => Ok(Self::Rounded),
            other => Err(ParseError(format!("unknown border: {other}"))),
        }
    }
}

impl fmt::Display for Border {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::None => "none",
            Self::Plain => "plain",
            Self::Rounded => "rounded",
        })
    }
}

/// A fixed size wraps a `u16` row/column count losslessly.
impl From<u16> for Size {
    fn from(n: u16) -> Self {
        Self::Fixed(n)
    }
}

impl FromStr for Size {
    type Err = ParseError;

    /// Parses only the `"fill"` keyword; fixed sizes use [`Size::from(u16)`].
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "fill" => Ok(Self::Fill),
            other => Err(ParseError(format!(
                "unknown size: {other} (expected \"fill\")"
            ))),
        }
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fill => f.write_str("fill"),
            Self::Fixed(n) => write!(f, "{n}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_forms_round_trip() {
        for (kind, s) in [
            (BufferKind::Messages, "messages"),
            (BufferKind::Input, "input"),
        ] {
            assert_eq!(kind.to_string(), s);
            assert_eq!(s.parse::<BufferKind>().expect("parses"), kind);
        }
        for (split, s) in [
            (Split::Top, "top"),
            (Split::Bottom, "bottom"),
            (Split::Left, "left"),
            (Split::Right, "right"),
        ] {
            assert_eq!(split.to_string(), s);
            assert_eq!(s.parse::<Split>().expect("parses"), split);
        }
        for (border, s) in [
            (Border::None, "none"),
            (Border::Plain, "plain"),
            (Border::Rounded, "rounded"),
        ] {
            assert_eq!(border.to_string(), s);
            assert_eq!(s.parse::<Border>().expect("parses"), border);
        }
    }

    #[test]
    fn size_parses_fill_and_from_u16() {
        assert_eq!("fill".parse::<Size>().expect("parses"), Size::Fill);
        assert_eq!(Size::from(3), Size::Fixed(3));
        assert_eq!(Size::Fixed(3).to_string(), "3");
        assert!("3".parse::<Size>().is_err());
    }
}
