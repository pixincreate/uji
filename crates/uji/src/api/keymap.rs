use std::rc::Rc;

use mlua::{Function, Lua, Table, Value};
use uji_ui::keymap::{Binding, Chord, Mode, describe};

use crate::api::Api;
use crate::api::bind::bind;

fn binding_from_lua(value: &Value) -> Option<Binding> {
    match value {
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

pub(crate) fn add(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(
        lua,
        api,
        move |api, _, (mode, key, binding): (String, String, Value)| {
            let (mode, chord) = target(&mode, &key)?;
            let binding = match binding {
                Value::Function(handler) => {
                    let name = format!("{} {}", mode.name(), describe(chord));
                    api.actions().borrow_mut().add(name.clone(), handler);
                    Binding::Action(name)
                }
                other => binding_from_lua(&other).ok_or_else(|| {
                    mlua::Error::runtime(
                        "binding must be an action name, a function, or { command = \"...\" }",
                    )
                })?,
            };
            api.keymap().borrow_mut().set(mode, chord, binding);
            Ok(())
        },
    )
}

pub(crate) fn remove(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, (mode, key): (String, String)| {
        let (mode, chord) = target(&mode, &key)?;
        api.keymap().borrow_mut().set(mode, chord, Binding::Unbound);
        Ok(())
    })
}

pub(crate) fn reset(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, ()| {
        api.keymap().borrow_mut().reset();
        Ok(())
    })
}

pub(crate) fn list(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, lua, ()| {
        let out = lua.create_table()?;
        let entries: Vec<_> = api
            .keymap()
            .borrow()
            .entries()
            .map(|(&(mode, chord), binding)| (mode, chord, binding.clone()))
            .collect();
        for (mode, chord, binding) in entries {
            let row = lua.create_table()?;
            row.set("mode", mode.name())?;
            row.set("key", describe(chord))?;
            match binding {
                Binding::Action(name) => row.set("action", name)?,
                Binding::Command(name) => row.set("command", name)?,
                Binding::Unbound => row.set("unbound", true)?,
            }
            out.push(row)?;
        }
        Ok(out)
    })
}

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let keymap = lua.create_table()?;
    keymap.set("add", add(lua, api)?)?;
    keymap.set("remove", remove(lua, api)?)?;
    keymap.set("reset", reset(lua, api)?)?;
    keymap.set("list", list(lua, api)?)?;
    Ok(keymap)
}
