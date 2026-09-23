use mlua::{IntoLua, Lua, Value};
use uji_agent::fs::{Excerpt, Files, Written};

use super::reply::Reply;
use crate::api::fs::FsOp;

struct Bytes(Vec<u8>);

struct Page(Excerpt);

struct Wrote(Written);

impl IntoLua for Bytes {
    fn into_lua(self, lua: &Lua) -> mlua::Result<Value> {
        lua.create_string(&self.0).map(Value::String)
    }
}

impl IntoLua for Page {
    fn into_lua(self, lua: &Lua) -> mlua::Result<Value> {
        let Excerpt { lines, cut, total } = self.0;
        let table = lua.create_table()?;
        table.set("lines", lines)?;
        table.set(
            "cut",
            lua.create_table_from(cut.into_iter().map(|at| (at, true)))?,
        )?;
        table.set("total", total)?;
        Ok(Value::Table(table))
    }
}

impl IntoLua for Wrote {
    fn into_lua(self, lua: &Lua) -> mlua::Result<Value> {
        let table = lua.create_table()?;
        table.set("created", self.0.created)?;
        Ok(Value::Table(table))
    }
}

pub(crate) fn run(id: u64, op: FsOp, files: &Files) -> Reply {
    match op {
        FsOp::Read { path } => Reply::new(id, files.read(&path).map(Bytes)),
        FsOp::Lines { path, window } => Reply::new(id, files.lines(&path, window).map(Page)),
        FsOp::Write { path, content } => Reply::new(id, files.write(&path, &content).map(Wrote)),
    }
}
