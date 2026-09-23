use std::rc::Rc;

use mlua::{Function, Lua, LuaSerdeExt, Table, Value};
use serde::Deserialize;
use uji_core::llm::Usage;
use uji_core::session::model::{Message, ToolCall};

use crate::api::Api;
use crate::api::bind::bind;
use crate::api::callbacks::canceller;
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
        count: usize,
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
        reasoning: Option<String>,
    },
    ToolResult {
        tool_call_id: String,
        name: String,
        content: String,
    },
    Done {
        text: String,
        reasoning: Option<String>,
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

fn report(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
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

fn steer(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, |api, lua, on_done: Function| {
        let id = api.callbacks().borrow_mut().open(on_done);
        api.request(Request::Steer(id));
        canceller(lua, api, id)
    })
}

fn approve(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
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

fn run_tool(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
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

fn compact(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
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

fn route(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, |api, lua, on_done: Function| {
        let id = api.callbacks().borrow_mut().open(on_done);
        api.request(Request::Route(id));
        canceller(lua, api, id)
    })
}

fn stream(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(
        lua,
        api,
        move |api, _, (wire, request, reply): (String, Table, Table)| {
            let stream =
                api.wires().borrow().get(&wire).cloned().ok_or_else(|| {
                    mlua::Error::runtime(format!("no wire is registered as {wire}"))
                })?;
            stream.call::<Value>((request, reply))
        },
    )
}

pub(crate) fn host(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let host = lua.create_table()?;
    host.set("report", report(lua, api)?)?;
    host.set("steer", steer(lua, api)?)?;
    host.set("approve", approve(lua, api)?)?;
    host.set("run_tool", run_tool(lua, api)?)?;
    host.set("compact", compact(lua, api)?)?;
    host.set("route", route(lua, api)?)?;
    host.set("stream", stream(lua, api)?)?;
    Ok(host)
}
