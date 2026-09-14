use std::sync::Arc;
use std::time::Instant;

use mlua::{LuaSerdeExt, Value as LuaValue};
use uji_core::llm::{
    AgentConfig, CancelToken, LuaToolSpec, StreamEvent, ToolDecision, ToolSpec, run_agent,
};
use uji_core::session::id::{MessageId, now_millis};
use uji_core::session::model::{Message, StoredMessage, ToolCall};
use uji_core::tools::policy::Action;
use uji_screen::model::RunState;

use super::{LoopData, ToolApproval};
use crate::runtime::events;
use crate::runtime::signal::Signal;

impl LoopData {
    pub(crate) fn submit(&mut self, text: &str) {
        if self.inner.state().borrow().run_state() == RunState::Working {
            self.queued.push_back(text.to_string());
            return;
        }
        if self.compact_if_needed() {
            self.queued.push_back(text.to_string());
            return;
        }
        self.app.clear_notices();
        self.app.reset_scroll();
        self.inner
            .emit(events::MESSAGE_SUBMITTED, &[("text", text.to_string())]);

        self.append(Message::User {
            text: text.to_string(),
        });
        self.maybe_title(text);

        let provider = self.inner.llm.borrow().clone();
        let model = self.inner.llm_model.borrow().clone();
        let client = Arc::clone(&self.inner.client);
        let context = {
            let conversation = self.app.messages();
            uji_core::llm::context::build(conversation.messages())
        };
        let system = {
            let state_rc = self.inner.state();
            let state = state_rc.borrow();
            uji_core::llm::system_prompt(
                state.opts().agent_system_prompt.as_deref(),
                &self.app.session().directory,
            )
        };
        let system = self.with_agent_context(system);
        let lua_tools = self.gather_lua_tools();
        let cwd = self.app.session().directory.clone();
        let sender = self.signals.clone();
        let cancel = CancelToken::new();
        self.cancel = Some(cancel.clone());
        {
            let state_rc = self.inner.state();
            let mut state = state_rc.borrow_mut();
            state.set_run_state(RunState::Working);
            state.set_turn_started(Some(Instant::now()));
        }
        self.inner.emit(events::STATUS_CHANGED, &[]);
        self.runtime.spawn(async move {
            let tools = uji_core::tools::builtin_registry();
            let config = AgentConfig {
                client: &client,
                provider: provider.as_ref(),
                model,
                system: Some(system),
                tools: &tools,
                lua_tools: &lua_tools,
                cwd: std::path::Path::new(&cwd),
                cancel,
            };
            let mut on_event = |event: StreamEvent| {
                let _ = sender.send(Signal::Llm(event));
            };
            run_agent(&config, context, &mut on_event).await;
        });
    }

    pub(crate) fn on_llm_event(&mut self, event: StreamEvent) {
        if matches!(
            event,
            StreamEvent::Cancelled | StreamEvent::Failed(_) | StreamEvent::Restarted { .. }
        ) {
            self.app.reveal_all();
            self.release_deferred();
            self.apply_llm_event(event);
            return;
        }
        let waiting = self.app.revealing() && !matches!(event, StreamEvent::Delta(_));
        if waiting || !self.deferred.is_empty() {
            self.deferred.push_back(event);
            return;
        }
        self.apply_llm_event(event);
    }

    pub(super) fn apply_llm_event(&mut self, event: StreamEvent) {
        match event {
            StreamEvent::Delta(delta) => self.app.append_pending(&delta),
            StreamEvent::Restarted { attempt, of, wait } => {
                self.app.take_pending();
                self.inner.report(format!(
                    "request failed, retrying in {}s ({attempt}/{of})",
                    wait.as_secs().max(1)
                ));
                self.drain_diagnostics();
                self.dirty = true;
            }
            StreamEvent::AssistantStep {
                text,
                tool_calls,
                reasoning_content,
            } => {
                self.app.take_pending();
                self.persist_assistant_step(text, tool_calls, reasoning_content);
            }
            StreamEvent::ToolResult {
                tool_call_id,
                name,
                content,
            } => {
                self.persist_tool_result(tool_call_id, name, content);
            }
            StreamEvent::ToolDecisionRequest {
                tool,
                subject,
                reply,
            } => {
                self.handle_tool_decision(&tool, &subject, reply);
            }
            StreamEvent::RunLuaTool {
                name,
                arguments,
                reply,
            } => {
                let result = self.run_lua_tool(&name, &arguments);
                let _ = reply.send(result);
                self.dirty = true;
            }
            StreamEvent::Done {
                text,
                reasoning_content,
            } => {
                self.finish_assistant(&text, reasoning_content);
                self.stop_working();
                self.maybe_submit_queued();
            }
            StreamEvent::Usage(usage) => {
                self.inner.api.session().add_usage(usage);
                self.inner.emit(events::STATUS_CHANGED, &[]);
            }
            StreamEvent::Cancelled => {
                self.app.take_pending();
                self.fail_assistant("interrupted");
                self.stop_working();
                self.queued.clear();
            }
            StreamEvent::Failed(err) => {
                self.app.take_pending();
                self.fail_assistant(&err);
                self.stop_working();
                self.maybe_submit_queued();
            }
        }
    }

    pub(crate) fn interrupt(&mut self) -> bool {
        let Some(cancel) = self.cancel.take() else {
            return false;
        };
        cancel.cancel();
        self.dirty = true;
        true
    }

    pub(super) fn append(&mut self, message: Message) {
        let kind = message.type_name();
        let text = message.text().to_string();
        let id = self.app.session().id;
        let stored = match self.storage.append_message(&id, message.clone()) {
            Ok(stored) => stored,
            Err(err) => {
                self.inner
                    .report(format!("failed to persist {kind} message: {err}"));
                let seq = self
                    .app
                    .messages()
                    .messages()
                    .last()
                    .map_or(0, |last| last.seq)
                    .saturating_add(1);
                StoredMessage {
                    id: MessageId::new(),
                    seq,
                    time_created: now_millis(),
                    message,
                }
            }
        };
        self.app.conversation().borrow_mut().push(stored);
        self.inner.emit(
            events::MESSAGE_APPENDED,
            &[("type", kind.to_string()), ("text", text)],
        );
        self.dirty = true;
    }

    pub(super) fn resolve_tool_confirmation(&mut self, allow: bool) {
        if let Some((arguments, reply)) = self.pending_tool.take() {
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

    fn with_agent_context(&self, mut system: String) -> String {
        for (name, call) in self.inner.api.agent_context().calls() {
            match call.call::<Option<String>>(()) {
                Ok(Some(extra)) if !extra.trim().is_empty() => {
                    system.push_str("\n\n");
                    system.push_str(extra.trim());
                }
                Ok(_) => {}
                Err(err) => self.inner.report(format!("agent context {name}: {err}")),
            }
        }
        system
    }

    fn persist_assistant_step(
        &mut self,
        text: String,
        tool_calls: Vec<ToolCall>,
        reasoning_content: Option<String>,
    ) {
        if !tool_calls.is_empty() {
            let names = tool_calls
                .iter()
                .map(|call| call.name.as_str())
                .collect::<Vec<_>>()
                .join(",");
            self.inner.emit(events::TOOL_STARTED, &[("tools", names)]);
        }
        self.append(Message::Assistant {
            text,
            tool_calls,
            reasoning_content,
        });
    }

    fn persist_tool_result(&mut self, tool_call_id: String, name: String, content: String) {
        self.inner.emit(
            events::TOOL_FINISHED,
            &[("name", name.clone()), ("content", content.clone())],
        );
        self.append(Message::Tool {
            tool_call_id,
            name,
            content,
        });
    }

    fn handle_tool_decision(
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

        let decision = self.inner.api.dispatch_tool("tool_call", &event);
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
                let prompt = uji_core::tools::prompt::describe(&tool.name, &final_args);
                self.pending_tool = Some((final_args, reply));
                self.app
                    .open_confirm(title.unwrap_or(prompt.question), prompt.detail);
                self.dirty = true;
            }
        }
    }

    fn gather_lua_tools(&self) -> Vec<LuaToolSpec> {
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

    fn run_lua_tool(&self, name: &str, arguments: &str) -> String {
        let args: serde_json::Value =
            serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);
        let Ok(args) = self.inner.lua.to_value(&args) else {
            return String::from("error: failed to convert arguments");
        };
        let tools = self.inner.api.lua_tools();
        let tools = tools.borrow();
        let Some(tool) = tools.get(name) else {
            return format!("error: unknown tool {name}");
        };
        match tool.run.call::<String>(args) {
            Ok(text) => text,
            Err(err) => format!("error: {err}"),
        }
    }

    pub(super) fn stop_working(&mut self) {
        self.cancel = None;
        {
            let state_rc = self.inner.state();
            let mut state = state_rc.borrow_mut();
            state.set_run_state(RunState::Idle);
            state.set_turn_started(None);
        }
        self.inner.emit(events::STATUS_CHANGED, &[]);
    }

    pub(super) fn maybe_submit_queued(&mut self) {
        if let Some(text) = self.queued.pop_front() {
            self.submit(&text);
        }
    }

    fn fail_assistant(&mut self, error: &str) {
        self.append(Message::Error {
            text: error.to_string(),
        });
    }

    pub(super) fn finish_assistant(&mut self, text: &str, reasoning_content: Option<String>) {
        self.app.take_pending();
        self.append(Message::Assistant {
            text: text.to_string(),
            tool_calls: Vec::new(),
            reasoning_content,
        });
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
