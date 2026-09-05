//! The `uji.*` function bindings: each parses Lua args and delegates to the
//! canonical [`crate::api`] (or handles a Lua-only surface directly).

mod buffer;
mod event;
mod schedule;
mod window;

use std::rc::Rc;

use mlua::{Lua as LuaState, Table};

use crate::lua::Lua;
use crate::runtime::Inner;

/// Build the `uji` API table with all built-in functions registered.
pub(crate) fn register_all(lua: &LuaState, inner: &Rc<Inner>) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    let functions: Vec<Box<dyn Lua>> = vec![
        Box::new(buffer::CreateBuf {
            inner: inner.clone(),
        }),
        Box::new(window::OpenWin {
            inner: inner.clone(),
        }),
        Box::new(window::CloseWin {
            inner: inner.clone(),
        }),
        Box::new(schedule::Schedule {
            inner: inner.clone(),
        }),
        Box::new(event::On {
            inner: inner.clone(),
        }),
        Box::new(event::Emit {
            inner: inner.clone(),
        }),
        Box::new(event::Notify),
    ];
    for function in &functions {
        function.register(lua, &table)?;
    }
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_all_registers_the_builtin_keys() {
        let inner = Inner::new(mlua::Lua::new());
        // ast-grep-ignore: no-expect-in-lib
        let table = register_all(&inner.lua, &inner).expect("api table builds");
        for key in [
            "create_buf",
            "open_win",
            "close_win",
            "schedule",
            "on",
            "emit",
            "notify",
        ] {
            assert!(table.get::<mlua::Function>(key).is_ok(), "missing {key}");
        }
    }
}
