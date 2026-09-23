use mlua::{Function, IntoLua, Lua, LuaSerdeExt, Table, Value as LuaValue};
use uji_agent::llm::ToolSpec;
use uji_agent::tools::policy::Action;

use super::{Awaiting, LoopData, Running, ToolApproval};
use crate::api::tools;
use crate::runtime::events;

enum Started {
    Done(String),
    Pending(Option<Function>),
}

enum Decision {
    Allow(Table),
    Deny(String),
}

impl IntoLua for Decision {
    fn into_lua(self, lua: &Lua) -> mlua::Result<LuaValue> {
        let table = lua.create_table()?;
        match self {
            Self::Allow(arguments) => {
                table.set("allow", true)?;
                table.set("arguments", arguments)?;
            }
            Self::Deny(reason) => table.set("deny", reason)?,
        }
        Ok(LuaValue::Table(table))
    }
}

impl LoopData {
    pub(super) fn resolve_tool_confirmation(&mut self, allow: bool) {
        let waiting = self
            .awaiting
            .take_if(|awaiting| matches!(awaiting, Awaiting::Approval { .. }));
        if let Some(Awaiting::Approval { id, arguments }) = waiting {
            let decision = if allow {
                Decision::Allow(arguments)
            } else {
                Decision::Deny(String::from("user denied"))
            };
            self.answer(id, decision);
        }
        self.app.close_modal();
        self.dirty = true;
    }

    pub(super) fn handle_tool_decision(&mut self, id: u64, name: &str, arguments: Table) {
        let verdict = self.policy_verdict(name, &arguments);
        let decision = self.inner.lua.create_table().ok().and_then(|event| {
            let _ = event.set("name", name);
            let _ = event.set("arguments", arguments.clone());
            self.inner.api.ask(events::Event::ToolCall.name(), &event)
        });
        match parse_tool_decision(decision).unwrap_or(verdict) {
            ToolApproval::Allow => self.answer(id, Decision::Allow(arguments)),
            ToolApproval::Deny { reason } => self.answer(id, Decision::Deny(reason)),
            ToolApproval::Ask { title } => {
                let shown = self
                    .inner
                    .lua
                    .from_value::<serde_json::Value>(LuaValue::Table(arguments.clone()))
                    .unwrap_or_default();
                let prompt = uji_agent::tools::prompt::describe(name, &shown.to_string());
                self.awaiting = Some(Awaiting::Approval { id, arguments });
                self.app
                    .open_confirm(title.unwrap_or(prompt.question), prompt.detail);
                self.dirty = true;
            }
        }
    }

    fn policy_verdict(&self, name: &str, args: &Table) -> ToolApproval {
        let tool = self.inner.api.tools().borrow().get(name).cloned();
        let (subject, declared) = match tool {
            Some(tool) => match tool.subject(name, args) {
                Ok(subject) => (subject, tool.policy),
                Err(err) => {
                    self.inner.report(format!("{name} subject: {err}"));
                    return ToolApproval::Ask { title: None };
                }
            },
            None => (name.to_string(), None),
        };
        match self
            .inner
            .policy
            .borrow()
            .evaluate(name, &subject, declared)
        {
            Action::Allow => ToolApproval::Allow,
            Action::Deny => ToolApproval::Deny {
                reason: String::from("denied by policy"),
            },
            Action::Ask => ToolApproval::Ask { title: None },
        }
    }

    pub(super) fn gather_tools(&self) -> Vec<ToolSpec> {
        let access = self.inner.api.access().borrow();
        self.inner
            .api
            .tools()
            .borrow()
            .iter()
            .filter(|(name, _)| !access.disabled().contains(*name))
            .map(|(name, tool)| ToolSpec {
                name: name.clone(),
                description: tool.description.clone(),
                parameters: self
                    .inner
                    .lua
                    .from_value(tool.parameters.clone())
                    .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new())),
            })
            .collect()
    }

    pub(super) fn run_tool(&mut self, id: u64, name: String, arguments: LuaValue) {
        match self.start_tool(&name, arguments, id) {
            Ok(Started::Done(text)) => self.answer(id, text),
            Ok(Started::Pending(cancel)) => {
                self.awaiting = Some(Awaiting::Result(Running {
                    call: id,
                    name,
                    cancel,
                }));
            }
            Err(err) => self.answer(id, format!("error: {err}")),
        }
        self.dirty = true;
    }

    fn start_tool(&self, name: &str, arguments: LuaValue, call: u64) -> mlua::Result<Started> {
        let lua = &self.inner.lua;
        let tool = self
            .inner
            .api
            .tools()
            .borrow()
            .get(name)
            .cloned()
            .ok_or_else(|| mlua::Error::runtime(format!("unknown tool {name}")))?;
        if !tool.defer {
            return tool.run.call::<String>(arguments).map(Started::Done);
        }
        let done = tools::done(lua, &self.inner.api, call)?;
        let progress = tools::progress(lua, &self.inner.api, call)?;
        tool.run
            .call::<Option<Function>>((arguments, done, progress))
            .map(Started::Pending)
    }

    pub(super) fn finish_tool(&mut self, call: u64, text: String) {
        let finished = self.awaiting.take_if(
            |awaiting| matches!(awaiting, Awaiting::Result(running) if running.call == call),
        );
        if finished.is_some() {
            self.answer(call, text);
        }
    }

    pub(super) fn stop_pending(&mut self, id: u64) {
        if let Some(awaiting) = self.awaiting.take_if(|awaiting| awaiting.id() == id) {
            self.release(awaiting);
        }
    }

    pub(super) fn release(&mut self, awaiting: Awaiting) {
        match awaiting {
            Awaiting::Approval { .. } => self.app.close_modal(),
            Awaiting::Result(Running { name, cancel, .. }) => {
                self.app.overlay_mut().set_running(None);
                if let Some(cancel) = cancel
                    && let Err(err) = cancel.call::<()>(())
                {
                    self.inner.report(format!("{name}: {err}"));
                }
            }
        }
        self.dirty = true;
    }

    pub(super) fn tool_progress(&mut self, call: u64, line: String) {
        if let Some(Awaiting::Result(running)) = &self.awaiting
            && running.call == call
        {
            self.app
                .overlay_mut()
                .set_running(Some((running.name.clone(), line)));
            self.dirty = true;
        }
    }
}

fn parse_tool_decision(value: Option<LuaValue>) -> Option<ToolApproval> {
    let LuaValue::Table(table) = value? else {
        return None;
    };
    let text = |key: &str| table.get::<Option<String>>(key).ok().flatten();
    let flag = |key: &str| matches!(table.get::<bool>(key), Ok(true));

    if let Some(reason) = text("deny") {
        return Some(ToolApproval::Deny { reason });
    }
    if flag("allow") {
        return Some(ToolApproval::Allow);
    }
    let title = text("ask");
    if title.is_some() || flag("ask") {
        return Some(ToolApproval::Ask { title });
    }
    None
}
