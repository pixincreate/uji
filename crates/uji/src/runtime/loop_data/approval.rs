use mlua::{LuaSerdeExt, Value as LuaValue};
use uji_agent::llm::ToolDecision;
use uji_agent::llm::{LuaToolSpec, ToolSpec};
use uji_agent::session::model::ToolCall;
use uji_agent::tools::policy::Action;

use super::{Awaiting, LoopData, ToolApproval, ToolOutcome};
use crate::runtime::events;

impl LoopData {
    pub(super) fn resolve_tool_confirmation(&mut self, allow: bool) {
        let waiting = self
            .awaiting
            .take_if(|awaiting| matches!(awaiting, Awaiting::Approval { .. }));
        if let Some(Awaiting::Approval { arguments, reply }) = waiting {
            let decision = if allow {
                ToolDecision::Allow { arguments }
            } else {
                ToolDecision::Deny {
                    reason: String::from("user denied"),
                }
            };
            let _ = reply.send(decision);
        }
        self.app.close_modal();
        self.dirty = true;
    }
    pub(super) fn handle_tool_decision(
        &mut self,
        tool: &ToolCall,
        subject: &str,
        reply: tokio::sync::oneshot::Sender<ToolDecision>,
    ) {
        let args_json: serde_json::Value =
            serde_json::from_str(tool.arguments.as_str()).unwrap_or(serde_json::Value::Null);
        let prepared = self
            .inner
            .lua
            .to_value(&args_json)
            .ok()
            .and_then(|args| match args {
                LuaValue::Table(table) => Some(table),
                _ => None,
            })
            .zip(self.inner.lua.create_table().ok());
        let Some((args_table, event)) = prepared else {
            deny(reply, "could not prepare the tool call for review");
            return;
        };
        let _ = event.set("name", tool.name.clone());
        let _ = event.set("arguments", args_table.clone());

        let decision = self.inner.api.ask(events::TOOL_CALL, &event);
        let approval = if let Some(approval) = parse_tool_decision(decision) {
            approval
        } else {
            let policy = self.inner.policy.borrow();
            match policy.evaluate(tool.name.as_str(), subject) {
                Action::Allow => ToolApproval::Allow,
                Action::Deny => ToolApproval::Deny {
                    reason: String::from("denied by policy"),
                },
                Action::Ask => ToolApproval::Ask { title: None },
            }
        };

        let final_args: serde_json::Value = self
            .inner
            .lua
            .from_value(LuaValue::Table(args_table))
            .unwrap_or(args_json);
        let final_args = final_args.to_string();

        match approval {
            ToolApproval::Allow => {
                let _ = reply.send(ToolDecision::Allow {
                    arguments: final_args,
                });
            }
            ToolApproval::Deny { reason } => {
                let _ = reply.send(ToolDecision::Deny { reason });
            }
            ToolApproval::Ask { title } => {
                let prompt = uji_agent::tools::prompt::describe(&tool.name, &final_args);
                self.awaiting = Some(Awaiting::Approval {
                    arguments: final_args,
                    reply,
                });
                self.app
                    .open_confirm(title.unwrap_or(prompt.question), prompt.detail);
                self.dirty = true;
            }
        }
    }

    pub(super) fn gather_lua_tools(&self) -> Vec<LuaToolSpec> {
        let mut tools = Vec::new();
        for (name, tool) in self.inner.api.lua_tools().borrow().iter() {
            let parameters: serde_json::Value = self
                .inner
                .lua
                .from_value(tool.parameters.clone())
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            tools.push(LuaToolSpec {
                name: name.clone(),
                spec: ToolSpec {
                    name: name.clone(),
                    description: tool.description.clone(),
                    parameters,
                },
                subject: tool.subject.clone().unwrap_or_else(|| name.clone()),
            });
        }
        tools
    }

    pub(super) fn run_lua_tool(&self, name: &str, arguments: &str) -> ToolOutcome {
        let args: serde_json::Value =
            serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);
        let Ok(args) = self.inner.lua.to_value(&args) else {
            return ToolOutcome::Done(String::from("error: failed to convert arguments"));
        };
        let tools = self.inner.api.lua_tools();
        let tools = tools.borrow();
        let Some(tool) = tools.get(name) else {
            return ToolOutcome::Done(format!("error: unknown tool {name}"));
        };
        if !tool.defer {
            return match tool.run.call::<String>(args) {
                Ok(text) => ToolOutcome::Done(text),
                Err(err) => ToolOutcome::Done(format!("error: {err}")),
            };
        }
        let done = match crate::api::tools::completion(&self.inner.lua, &self.inner.api) {
            Ok(done) => done,
            Err(err) => return ToolOutcome::Done(format!("error: {err}")),
        };
        match tool.run.call::<()>((args, done)) {
            Ok(()) => ToolOutcome::Pending,
            Err(err) => ToolOutcome::Done(format!("error: {err}")),
        }
    }

    pub(super) fn finish_lua_tool(&mut self, text: String) {
        let waiting = self
            .awaiting
            .take_if(|awaiting| matches!(awaiting, Awaiting::Result(_)));
        match waiting {
            Some(Awaiting::Result(reply)) => {
                let _ = reply.send(text);
            }
            _ => self
                .inner
                .report(String::from("tool done() called with no tool waiting")),
        }
    }
}

fn deny(reply: tokio::sync::oneshot::Sender<ToolDecision>, reason: &str) {
    let _ = reply.send(ToolDecision::Deny {
        reason: reason.to_string(),
    });
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
