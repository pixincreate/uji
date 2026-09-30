use std::sync::atomic::{AtomicI64, Ordering};

use mlua::prelude::*;

static DROPPED: AtomicI64 = AtomicI64::new(0);

struct Counter {
    start: i64,
    value: i64,
    step: i64,
}

impl Drop for Counter {
    fn drop(&mut self) {
        DROPPED.fetch_add(1, Ordering::SeqCst);
    }
}

impl LuaUserData for Counter {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("start", |_, this| Ok(this.start));
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("bump", |_, this, ()| {
            this.value += this.step;
            Ok(this.value)
        });
    }
}

#[mlua::lua_module]
fn testmod(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set(
        "add",
        lua.create_function(|_, (a, b): (i64, i64)| Ok(a + b))?,
    )?;
    module.set(
        "greet",
        lua.create_function(|_, name: String| Ok(format!("hello {name}")))?,
    )?;
    module.set(
        "insist",
        lua.create_function(|_, reason: String| Err::<(), _>(LuaError::runtime(reason)))?,
    )?;
    module.set(
        "counter",
        lua.create_function(|_, (start, step): (i64, Option<i64>)| {
            Ok(Counter {
                start,
                value: start,
                step: step.unwrap_or(1),
            })
        })?,
    )?;
    module.set(
        "dropped",
        lua.create_function(|_, ()| Ok(DROPPED.load(Ordering::SeqCst)))?,
    )?;
    Ok(module)
}

#[mlua::lua_module(name = "uji_sys_sha256")]
fn sha256(lua: &Lua) -> LuaResult<LuaFunction> {
    lua.create_function(|_, text: String| Ok(format!("replaced {text}")))
}
