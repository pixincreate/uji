//! The static registry of `uji.*` function bindings.

mod buffer;
mod event;
mod schedule;
mod window;

use std::rc::Rc;

use mlua::{Function, Lua as LuaState, Table};

use crate::runtime::Inner;

/// One entry in the `uji` API registry: the key it's registered under, and
/// the function that builds the callable for a given runtime.
struct Api {
    key: &'static str,
    build: fn(&LuaState, &Rc<Inner>) -> mlua::Result<Function>,
}

/// Every function exposed on the `uji` table, as a static descriptor table —
/// nvim's API metadata style: no allocation, no dynamic dispatch.
static REGISTRY: &[Api] = &[
    Api {
        key: "create_buf",
        build: buffer::create_buf,
    },
    Api {
        key: "open_win",
        build: window::open_win,
    },
    Api {
        key: "close_win",
        build: window::close_win,
    },
    Api {
        key: "schedule",
        build: schedule::schedule,
    },
    Api {
        key: "on",
        build: event::on,
    },
    Api {
        key: "emit",
        build: event::emit,
    },
    Api {
        key: "notify",
        build: event::notify,
    },
];

/// Build the `uji` API table from the static registry.
pub(crate) fn register_all(lua: &LuaState, inner: &Rc<Inner>) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    for entry in REGISTRY {
        table.set(entry.key, (entry.build)(lua, inner)?)?;
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
            assert!(table.get::<Function>(key).is_ok(), "missing {key}");
        }
    }
}
