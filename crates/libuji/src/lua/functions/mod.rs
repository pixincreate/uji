mod buffer;
mod event;
mod schedule;
mod window;

use std::rc::Rc;

use mlua::{Function, Lua as LuaState, Table};

use crate::runtime::Inner;

struct Api {
    key: &'static str,
    build: fn(&LuaState, &Rc<Inner>) -> mlua::Result<Function>,
}

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

pub(crate) fn register_all(lua: &LuaState, inner: &Rc<Inner>) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    for entry in REGISTRY {
        table.set(entry.key, (entry.build)(lua, inner)?)?;
    }
    Ok(table)
}
