//! Lua → Rust conversions for the UI model.
//!
//! `mlua`'s own `FromLua` can't be implemented for `tui::model` types across
//! crates (orphan rule), so we define a small local analog. The string fields
//! delegate to the `FromStr` impls in `tui::convert`; numbers use `From` /
//! `TryFrom`.

use mlua::Value as LuaValue;
use tui::model::{Border, Size, Split, WinOpts};

/// Convert a Lua value into a Rust model type.
pub(crate) trait FromLuaValue: Sized {
    /// Convert `value` into `Self`.
    fn from_lua_value(value: &LuaValue) -> mlua::Result<Self>;
}

impl FromLuaValue for Size {
    fn from_lua_value(value: &LuaValue) -> mlua::Result<Self> {
        match value {
            LuaValue::Integer(n) => u16::try_from(*n)
                .map(Size::from)
                .map_err(|_| mlua::Error::runtime("size must fit in u16")),
            LuaValue::String(s) => s
                .to_str()?
                .parse::<Size>()
                .map_err(|err| mlua::Error::runtime(err.to_string())),
            _ => Err(mlua::Error::runtime("size must be a number or \"fill\"")),
        }
    }
}

impl FromLuaValue for WinOpts {
    fn from_lua_value(value: &LuaValue) -> mlua::Result<Self> {
        let LuaValue::Table(table) = value else {
            return Err(mlua::Error::runtime("window options must be a table"));
        };
        let mut opts = WinOpts::default();
        if let Some(split) = table.get::<Option<String>>("split")? {
            opts.split = split
                .parse::<Split>()
                .map_err(|err| mlua::Error::runtime(err.to_string()))?;
        }
        if let Some(size) = table.get::<Option<LuaValue>>("size")? {
            opts.size = Size::from_lua_value(&size)?;
        }
        if let Some(border) = table.get::<Option<String>>("border")? {
            opts.border = border
                .parse::<Border>()
                .map_err(|err| mlua::Error::runtime(err.to_string()))?;
        }
        opts.title = table.get::<Option<String>>("title")?;
        Ok(opts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;

    #[test]
    fn size_from_lua_accepts_number_and_fill() {
        let lua = Lua::new();
        let int = lua.load("3").eval::<LuaValue>().expect("value");
        assert_eq!(Size::from_lua_value(&int).expect("fixed"), Size::Fixed(3));

        let fill = lua.load("\"fill\"").eval::<LuaValue>().expect("value");
        assert_eq!(Size::from_lua_value(&fill).expect("fill"), Size::Fill);
    }

    #[test]
    fn win_opts_from_lua_table() {
        let lua = Lua::new();
        let value = lua
            .load("{ split = \"bottom\", size = 3, border = \"rounded\", title = \" t \" }")
            .eval::<LuaValue>()
            .expect("table");
        let opts = WinOpts::from_lua_value(&value).expect("opts");
        assert_eq!(opts.split, Split::Bottom);
        assert_eq!(opts.size, Size::Fixed(3));
        assert_eq!(opts.border, Border::Rounded);
        assert_eq!(opts.title.as_deref(), Some(" t "));
    }
}
