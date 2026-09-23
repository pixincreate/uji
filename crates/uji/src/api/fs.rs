use std::rc::Rc;

use mlua::{Function, Lua, LuaString, Table};
use uji_agent::fs::Window;

use crate::api::Api;
use crate::api::bind::bind;
use crate::api::request::Request;

pub enum FsOp {
    Read { path: String },
    Lines { path: String, window: Window },
    Write { path: String, content: Vec<u8> },
}

fn window(opts: &Table) -> mlua::Result<Window> {
    Ok(Window {
        offset: opts.get::<Option<usize>>("offset")?.unwrap_or(1).max(1),
        limit: opts.get::<Option<usize>>("limit")?.unwrap_or(usize::MAX),
        max_line: opts.get::<Option<usize>>("max_line")?.unwrap_or(usize::MAX),
    })
}

fn start(api: &Api, op: FsOp, on_done: Function) {
    let id = api.callbacks().borrow_mut().open(on_done);
    api.request(Request::Fs { id, op });
}

pub(crate) fn read(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, |api, _, (path, on_done): (String, Function)| {
        start(api, FsOp::Read { path }, on_done);
        Ok(())
    })
}

pub(crate) fn lines(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(
        lua,
        api,
        |api, _, (path, opts, on_done): (String, Table, Function)| {
            let window = window(&opts)?;
            start(api, FsOp::Lines { path, window }, on_done);
            Ok(())
        },
    )
}

pub(crate) fn write(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(
        lua,
        api,
        |api, _, (path, content, on_done): (String, LuaString, Function)| {
            let content = content.as_bytes().to_vec();
            start(api, FsOp::Write { path, content }, on_done);
            Ok(())
        },
    )
}

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let fs = lua.create_table()?;
    fs.set("read", read(lua, api)?)?;
    fs.set("lines", lines(lua, api)?)?;
    fs.set("write", write(lua, api)?)?;
    Ok(fs)
}
