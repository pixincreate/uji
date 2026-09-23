use std::rc::Rc;

use mlua::{Function, Lua, Table, Value};

use crate::api::Api;
use crate::api::bind::bind;
use crate::api::registry::{DEFAULT_PRIORITY, Entry};
use uji_ui::model::RunState;

pub fn add(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(
        lua,
        api,
        move |api, _, (name, render, opts): (String, Function, Option<Table>)| {
            let priority = opts
                .map(|opts| opts.get::<Option<i64>>("priority"))
                .transpose()?
                .flatten()
                .unwrap_or(DEFAULT_PRIORITY);
            api.segments().borrow_mut().add(Entry {
                name,
                priority,
                call: render,
            });
            Ok(())
        },
    )
}

pub fn remove(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, name: String| {
        Ok(api.segments().borrow_mut().remove(&name))
    })
}

pub fn list(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, ()| {
        Ok(api.segments().borrow().names())
    })
}

/// Calls every registered segment and returns the ones that produced something.
pub fn render(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, lua, ()| {
        let out = lua.create_table()?;
        let segments = api.segments().borrow().calls();
        for (name, render) in segments {
            match render.call::<Value>(()) {
                Ok(Value::Nil) => {}
                Ok(value) => out.push(value)?,
                Err(err) => {
                    api.notify(format!("status segment {name}: {err}"));
                }
            }
        }
        Ok(out)
    })
}

pub fn provider(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, |api, _, ()| {
        Ok(api.state().borrow().current_provider().map(str::to_string))
    })
}

pub fn model(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, |api, _, ()| {
        Ok(api.state().borrow().current_model().map(str::to_string))
    })
}

pub fn effort(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, |api, _, ()| {
        Ok(api.state().borrow().current_effort().map(str::to_string))
    })
}

pub fn queue(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, |api, _, ()| {
        Ok(api.state().borrow().queued().to_vec())
    })
}

pub fn context(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, lua, ()| {
        let out = lua.create_table()?;
        out.set("used", api.conversation().borrow().used_tokens())?;
        out.set("window", api.state().borrow().context_window())?;
        Ok(out)
    })
}

pub fn state(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, |api, _, ()| {
        let run_state = api.state().borrow().run_state();
        Ok(match run_state {
            RunState::Idle => "idle",
            RunState::Working => "working",
        })
    })
}

pub fn elapsed(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, |api, _, ()| {
        Ok(api
            .state()
            .borrow()
            .turn_started()
            .map(|started| started.elapsed().as_secs_f64()))
    })
}

pub fn loader_frame(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, |api, _, ()| {
        Ok(api.state().borrow().loader_frame())
    })
}
