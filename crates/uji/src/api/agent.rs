use std::rc::Rc;

use mlua::{Function, Lua, LuaSerdeExt, Table, Value};
use serde::Deserialize;
use uji_agent::llm::Usage;
use uji_agent::session::model::{Message, ToolCall};

use crate::api::Api;
use crate::api::bind::bind;
use crate::api::callbacks::canceller;
use crate::api::registry::Entry;
use crate::api::request::Request;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Report {
    Text {
        text: String,
    },
    Reasoning {
        text: String,
    },
    Usage(Usage),
    Compacted {
        usage: Option<Usage>,
    },
    Restarted {
        attempt: u32,
        of: u32,
        wait: u64,
    },
    AssistantStep {
        text: String,
        #[serde(default)]
        tool_calls: Vec<ToolCall>,
        reasoning_content: Option<String>,
    },
    ToolResult {
        tool_call_id: String,
        name: String,
        content: String,
    },
    Done {
        text: String,
        reasoning_content: Option<String>,
    },
    Cancelled,
    Failed {
        message: String,
    },
}

impl Report {
    pub fn is_delta(&self) -> bool {
        matches!(self, Self::Text { .. } | Self::Reasoning { .. })
    }
}

pub(crate) fn context(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(
        lua,
        api,
        move |api, _, (name, call, opts): (String, Function, Option<Table>)| {
            let priority = opts
                .map(|opts| opts.get::<Option<i64>>("priority"))
                .transpose()?
                .flatten()
                .unwrap_or(50);
            api.agent_context().borrow_mut().add(Entry {
                name,
                priority,
                call,
            });
            Ok(())
        },
    )
}

pub(crate) fn clear_context(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, name: String| {
        api.agent_context().borrow_mut().remove(&name);
        Ok(())
    })
}

pub(crate) fn contexts(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, lua, ()| {
        let out = lua.create_table()?;
        for (name, call) in api.agent_context().borrow().calls() {
            match call.call::<Value>(()) {
                Ok(Value::Nil) => {}
                Ok(value) => out.push(value)?,
                Err(err) => api.notify(format!("agent context {name}: {err}")),
            }
        }
        Ok(out)
    })
}

pub(crate) fn report(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(
        lua,
        api,
        |api, lua, (event, on_applied): (Value, Option<Function>)| {
            let event = lua.from_value::<Report>(event)?;
            api.request(Request::Report { event, on_applied });
            Ok(())
        },
    )
}

pub(crate) fn steer(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, |api, lua, on_done: Function| {
        let id = api.callbacks().borrow_mut().open(on_done);
        api.request(Request::Steer(id));
        canceller(lua, api, id)
    })
}

pub(crate) fn approve(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, |api, lua, (call, on_done): (Table, Function)| {
        let name = call.get::<String>("name")?;
        let arguments = call.get::<Table>("arguments")?;
        let id = api.callbacks().borrow_mut().open(on_done);
        api.request(Request::Approve {
            id,
            name,
            arguments,
        });
        canceller(lua, api, id)
    })
}

pub(crate) fn run_tool(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(
        lua,
        api,
        |api, lua, (name, arguments, on_done): (String, Value, Function)| {
            let id = api.callbacks().borrow_mut().open(on_done);
            api.request(Request::RunTool {
                id,
                name,
                arguments,
            });
            canceller(lua, api, id)
        },
    )
}

pub(crate) fn compact(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(
        lua,
        api,
        |api, lua, (messages, on_done): (Value, Function)| {
            let messages = lua.from_value::<Vec<Message>>(messages)?;
            let id = api.callbacks().borrow_mut().open(on_done);
            api.request(Request::Compact { id, messages });
            canceller(lua, api, id)
        },
    )
}

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let agent = lua.create_table()?;
    agent.set("report", report(lua, api)?)?;
    agent.set("steer", steer(lua, api)?)?;
    agent.set("approve", approve(lua, api)?)?;
    agent.set("run_tool", run_tool(lua, api)?)?;
    agent.set("compact", compact(lua, api)?)?;
    agent.set("context", context(lua, api)?)?;
    agent.set("clear_context", clear_context(lua, api)?)?;
    agent.set("contexts", contexts(lua, api)?)?;
    Ok(agent)
}
