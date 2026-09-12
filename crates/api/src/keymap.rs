use std::rc::Rc;

use mlua::{Function, Lua, Table, Value};
use uji_screen::keymap::{Binding, Chord, Mode, describe};

use crate::Api;

fn binding_from_lua(value: &Value) -> Option<Binding> {
    match value {
        Value::Nil => Some(Binding::Unbound),
        Value::String(action) => Some(Binding::Action(action.to_string_lossy())),
        Value::Table(table) => {
            if let Ok(Some(command)) = table.get::<Option<String>>("command") {
                return Some(Binding::Command(command));
            }
            if let Ok(Some(action)) = table.get::<Option<String>>("action") {
                return Some(Binding::Action(action));
            }
            None
        }
        _ => None,
    }
}

fn target(mode: &str, key: &str) -> mlua::Result<(Mode, Chord)> {
    let parsed_mode = Mode::parse(mode)
        .ok_or_else(|| mlua::Error::runtime(format!("unknown keymap mode: {mode}")))?;
    let chord = Chord::parse(key)
        .ok_or_else(|| mlua::Error::runtime(format!("cannot parse key: {key}")))?;
    Ok((parsed_mode, chord))
}

pub(crate) fn set(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(move |_, (mode, key, binding): (String, String, Value)| {
        let (mode, chord) = target(&mode, &key)?;
        let binding = binding_from_lua(&binding).ok_or_else(|| {
            mlua::Error::runtime("binding must be an action name, { command = \"...\" }, or nil")
        })?;
        api.keymap().borrow_mut().set(mode, chord, binding);
        Ok(())
    })
}

pub(crate) fn del(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(move |_, (mode, key): (String, String)| {
        let (mode, chord) = target(&mode, &key)?;
        api.keymap().borrow_mut().set(mode, chord, Binding::Unbound);
        Ok(())
    })
}

pub(crate) fn reset(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(move |_, ()| {
        api.keymap().borrow_mut().reset();
        Ok(())
    })
}

pub(crate) fn list(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(move |lua, ()| {
        let out = lua.create_table()?;
        for ((mode, chord), binding) in api.keymap().borrow().entries() {
            let row = lua.create_table()?;
            row.set("mode", mode.name())?;
            row.set("key", describe(*chord))?;
            match binding {
                Binding::Action(name) => row.set("action", name.clone())?,
                Binding::Command(name) => row.set("command", name.clone())?,
                Binding::Unbound => row.set("unbound", true)?,
            }
            out.push(row)?;
        }
        Ok(out)
    })
}

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let keymap = lua.create_table()?;
    keymap.set("set", set(lua, api)?)?;
    keymap.set("del", del(lua, api)?)?;
    keymap.set("reset", reset(lua, api)?)?;
    keymap.set("list", list(lua, api)?)?;
    Ok(keymap)
}
