use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{Event as TermEvent, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use uji_screen::keymap::{Binding, Chord, Key};
use uji_screen::model::RunState;

use crate::cmd::{Args, Context};
use mlua::{LuaSerdeExt, Value as LuaValue};
use uji_core::credential;
use uji_core::llm::{
    AgentConfig, CancelToken, LuaToolSpec, StreamEvent, ToolDecision, ToolSpec, run_agent,
};
use uji_core::session::model::{Message, ToolCall};
use uji_core::session::store::SessionStorage;
use uji_core::tools::policy::Action;
use uji_tui::app::{Action as KeyBinding, App, KeyAction, SuggestItem};

use super::Inner;
use super::auth::AuthEvent;
use super::background::{Background, TitleEvent};
use super::builtin::Builtin;
use super::events;
use super::frontend::Frontend;
use super::job::{JobEvent, Running};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Control {
    Run,
    Reload,
    Quit,
}

enum ModalInput {
    Select(String),
    Prompt(String),
    Cancel,
}

enum ToolApproval {
    Allow,
    Deny { reason: String },
    Ask { title: Option<String> },
}

pub(crate) struct LoopData {
    pub(crate) inner: Rc<Inner>,
    pub(crate) app: App,
    pub(crate) storage: Box<dyn SessionStorage>,
    pub(crate) frontend: Box<dyn Frontend>,
    pub(crate) dirty: bool,
    pub(crate) control: Control,
    pub(crate) llm_tx: calloop::channel::Sender<StreamEvent>,
    pub(crate) runtime: tokio::runtime::Runtime,
    pub(crate) active: Option<Builtin>,
    pub(crate) action_done: bool,
    pub(crate) pending_tool: Option<(String, tokio::sync::oneshot::Sender<ToolDecision>)>,
    pub(crate) queued: VecDeque<String>,
    pub(crate) cancel: Option<CancelToken>,
    pub(crate) config_dir: Option<PathBuf>,
    pub(crate) job_tx: calloop::channel::Sender<JobEvent>,
    pub(crate) background_tx: calloop::channel::Sender<Background>,
    pub(crate) jobs: Running,
    pub(crate) reader_paused: Arc<std::sync::atomic::AtomicBool>,
}

impl LoopData {
    pub(crate) fn on_term_event(&mut self, event: &TermEvent) {
        match event {
            TermEvent::Key(key) => {
                let Some(action) = self.dispatch_key(*key) else {
                    self.dirty = true;
                    return;
                };
                match action {
                    KeyAction::Quit => self.control = Control::Quit,
                    KeyAction::Submit(text) => self.submit(&text),
                    KeyAction::Command(command) => self.on_command(&command),
                    KeyAction::Selected(item) => {
                        if self.pending_tool.is_some() {
                            self.resolve_tool_confirmation(item == "allow");
                        } else {
                            self.on_modal(ModalInput::Select(item));
                        }
                    }
                    KeyAction::Prompted(value) => self.on_modal(ModalInput::Prompt(value)),
                    KeyAction::Cancel => {
                        if self.pending_tool.is_some() {
                            self.resolve_tool_confirmation(false);
                        } else {
                            self.on_modal(ModalInput::Cancel);
                        }
                    }
                    KeyAction::Interrupt => {
                        if !self.interrupt() {
                            self.control = Control::Quit;
                        }
                    }
                    KeyAction::None => {}
                }
                self.dirty = true;
            }
            TermEvent::Mouse(mouse) => {
                match mouse.kind {
                    MouseEventKind::ScrollUp => self.app.scroll_up(3),
                    MouseEventKind::ScrollDown => self.app.scroll_down(3),
                    _ => return,
                }
                self.dirty = true;
            }
            TermEvent::Resize(..) => self.dirty = true,
            _ => {}
        }
    }

    fn dispatch_key(&mut self, key: KeyEvent) -> Option<KeyAction> {
        if self.inner.api.capture().is_active() {
            self.dispatch_capture(key);
            return None;
        }
        let mode = self.app.keymap_mode();
        let binding = chord_of(key)
            .and_then(|chord| self.inner.api.keymap().borrow().get(mode, chord).cloned());
        match binding {
            Some(Binding::Unbound) => None,
            Some(Binding::Command(command)) => {
                self.on_command(&command);
                None
            }
            Some(Binding::Action(name)) => {
                if let Some(action) = KeyBinding::parse(&name) {
                    return Some(self.app.apply(action));
                }
                match self.inner.api.actions().get(&name) {
                    Some(handler) => {
                        if let Err(err) = handler.call::<()>(()) {
                            self.inner.report(format!("action {name}: {err}"));
                        }
                        self.dirty = true;
                    }
                    None => self.inner.report(format!("unknown keymap action: {name}")),
                }
                None
            }
            None => Some(self.app.handle_key(key)),
        }
    }

    fn dispatch_capture(&mut self, key: KeyEvent) {
        let Some(handler) = self.inner.api.capture().handler() else {
            return;
        };
        let Some(chord) = chord_of(key) else {
            return;
        };
        let Ok(event) = self.inner.lua.create_table() else {
            return;
        };
        let _ = event.set("key", uji_screen::keymap::describe(chord));
        if let Key::Char(c) = chord.key {
            let _ = event.set("char", c.to_string());
        }
        let _ = event.set("ctrl", chord.ctrl);
        let _ = event.set("alt", chord.alt);
        let _ = event.set("shift", chord.shift);
        if let Err(err) = handler.call::<()>((event,)) {
            self.inner.report(format!("capture handler: {err}"));
            self.inner.api.capture().clear();
        }
        self.dirty = true;
    }

    pub(crate) fn submit(&mut self, text: &str) {
        if self.inner.state().borrow().run_state() == RunState::Working {
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
            uji_core::llm::context::sanitize(conversation.messages())
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
        let sender = self.llm_tx.clone();
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
                let _ = sender.send(event);
            };
            run_agent(&config, context, &mut on_event).await;
        });
    }

    pub(crate) fn on_llm_event(&mut self, event: StreamEvent) {
        match event {
            StreamEvent::Delta(delta) => {
                self.app.append_pending(&delta);
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

    pub(crate) fn pump(
        &mut self,
        handle: &calloop::LoopHandle<'static, Self>,
    ) -> std::io::Result<()> {
        const DRAINS: &[fn(&mut LoopData)] = &[
            LoopData::apply_composer,
            LoopData::drain_submits,
            LoopData::drain_titles,
            LoopData::drain_jobs,
            LoopData::drain_exec,
            LoopData::drain_diagnostics,
        ];
        for drain in DRAINS {
            drain(self);
        }

        for callback in self.inner.api.scheduled().take() {
            let _ = handle.insert_idle(move |data: &mut LoopData| {
                if let Err(err) = callback.call::<()>(()) {
                    data.inner
                        .report(format!("scheduled callback error: {err}"));
                }
                data.dirty = true;
            });
        }

        if self.control == Control::Reload {
            self.perform_reload();
        }

        if self.dirty {
            self.frontend.draw(&self.app)?;
            self.dirty = false;
        }
        Ok(())
    }

    pub(crate) fn apply_composer(&mut self) {
        let composer = self.inner.api.composer();
        if let Some(text) = composer.take_written() {
            self.app.set_input(text);
            self.dirty = true;
        } else {
            composer.observe(self.app.input());
        }
    }

    pub(crate) fn drain_titles(&mut self) {
        let Some(title) = self.inner.api.session().take_titles().pop() else {
            return;
        };
        self.set_title(title);
    }

    pub(crate) fn drain_submits(&mut self) {
        for text in self.inner.api.session().take_submits() {
            self.submit(&text);
        }
    }

    fn append(&mut self, message: Message) {
        let kind = message.type_name();
        let text = message.text().to_string();
        let id = self.app.session().id;
        match self.storage.append_message(&id, message) {
            Ok(stored) => {
                self.app.conversation().borrow_mut().push(stored);
                self.inner.emit(
                    events::MESSAGE_APPENDED,
                    &[("type", kind.to_string()), ("text", text)],
                );
            }
            Err(err) => self
                .inner
                .report(format!("failed to persist {kind} message: {err}")),
        }
        self.dirty = true;
    }

    pub(crate) fn drain_exec(&mut self) {
        for command in self.inner.api.take_exec() {
            let Some((program, args)) = command.split_first() else {
                continue;
            };
            self.reader_paused
                .store(true, std::sync::atomic::Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(120));
            if let Err(err) = self.frontend.suspend() {
                self.inner.report(format!("suspend terminal: {err}"));
            }
            let status = std::process::Command::new(program).args(args).status();
            if let Err(err) = self.frontend.resume() {
                self.inner.report(format!("resume terminal: {err}"));
            }
            self.reader_paused
                .store(false, std::sync::atomic::Ordering::Relaxed);
            match status {
                Ok(status) if !status.success() => {
                    self.inner.report(format!("{program} exited with {status}"));
                }
                Err(err) => self.inner.report(format!("run {program}: {err}")),
                Ok(_) => {}
            }
            self.dirty = true;
        }
    }

    pub(crate) fn on_background(&mut self, event: Background) {
        match event {
            Background::Auth(event) => self.on_auth_event(event),
            Background::Title(event) => self.on_title_event(event),
        }
    }

    fn on_title_event(&mut self, event: TitleEvent) {
        let TitleEvent::Ready { title, usage } = event else {
            return;
        };
        if let Some(usage) = usage {
            self.app.conversation().borrow_mut().add_usage(usage);
        }
        self.set_title(title);
    }

    fn set_title(&mut self, title: String) {
        let id = self.app.session().id;
        if let Err(err) = self.storage.rename_session(&id, &title) {
            self.inner
                .report(format!("could not save the session title: {err}"));
            return;
        }
        self.app.set_title(title.clone());
        self.app
            .conversation()
            .borrow_mut()
            .set_title(title.clone());
        self.inner.emit(events::SESSION_TITLED, &[("title", title)]);
        self.dirty = true;
    }

    fn maybe_title(&mut self, first_message: &str) {
        if !self.app.session().is_untitled() {
            return;
        }
        if self.app.conversation().borrow().messages().len() != 1 {
            return;
        }
        super::background::title(
            &self.runtime,
            Arc::clone(&self.inner.client),
            self.inner.llm.borrow().clone(),
            self.inner.llm_model.borrow().clone(),
            first_message.to_string(),
            self.background_tx.clone(),
        );
    }

    fn on_auth_event(&mut self, event: AuthEvent) {
        match event {
            AuthEvent::Opened { url } => {
                self.inner
                    .report(format!("opened your browser to sign in - {url}"));
            }
            AuthEvent::Done { provider_id } => {
                self.inner.report(format!("signed in to {provider_id}"));
                self.inner.resolve_llm(&mut *self.storage);
                self.inner.emit(events::STATUS_CHANGED, &[]);
            }
            AuthEvent::Failed { message } => {
                self.inner.report(format!("sign-in failed: {message}"));
            }
        }
        self.drain_diagnostics();
        self.dirty = true;
    }

    pub(crate) fn drain_jobs(&mut self) {
        let (requests, stops) = {
            let jobs = self.inner.api.jobs();
            let mut jobs = jobs.borrow_mut();
            (jobs.take_requests(), jobs.take_stops())
        };
        for id in stops {
            self.jobs.stop(id);
        }
        for request in requests {
            let cancel = CancelToken::new();
            self.jobs.insert(request.id, cancel.clone());
            let sender = self.job_tx.clone();
            self.runtime.spawn(super::job::run(
                request.id,
                request.command,
                request.cwd,
                cancel,
                move |event| {
                    let _ = sender.send(event);
                },
            ));
        }
    }

    pub(crate) fn on_job_event(&mut self, event: &JobEvent) {
        let exited = matches!(event, JobEvent::Exit { .. });
        let id = event.id();
        let arg = match event {
            JobEvent::Stdout { line, .. } | JobEvent::Stderr { line, .. } => self.lua_text(line),
            JobEvent::Exit { code, .. } => LuaValue::Integer(i64::from(*code)),
        };
        let callback = {
            let jobs = self.inner.api.jobs();
            let jobs = jobs.borrow();
            jobs.handlers(id).and_then(|handlers| match event {
                JobEvent::Stdout { .. } => handlers.on_stdout.clone(),
                JobEvent::Stderr { .. } => handlers.on_stderr.clone(),
                JobEvent::Exit { .. } => handlers.on_exit.clone(),
            })
        };
        if let Some(callback) = callback
            && let Err(err) = callback.call::<()>((arg,))
        {
            self.inner.report(format!("job {id}: {err}"));
        }
        if exited {
            self.inner.api.jobs().borrow_mut().finish(id);
            self.jobs.finish(id);
        }
        self.dirty = true;
    }

    fn lua_text(&self, text: &str) -> LuaValue {
        self.inner
            .lua
            .create_string(text)
            .map_or(LuaValue::Nil, LuaValue::String)
    }

    pub(crate) fn drain_diagnostics(&mut self) {
        let notices = self.inner.take_diagnostics();
        if !notices.is_empty() {
            self.app.push_notices(notices);
            self.dirty = true;
        }
    }

    pub(crate) fn perform_reload(&mut self) {
        self.control = Control::Run;
        let state = self.inner.state();
        state.borrow_mut().clear();
        let client = Arc::clone(&self.inner.client);
        let conversation = Rc::clone(self.app.conversation());
        self.inner = Inner::boot(state, conversation, client, self.config_dir.clone());
        self.inner.resolve_llm(&mut *self.storage);
        self.refresh_suggestions();
        self.drain_diagnostics();
        self.inner.emit(events::STATUS_CHANGED, &[]);
        self.dirty = true;
    }

    pub(crate) fn interrupt(&mut self) -> bool {
        if self.inner.state().borrow().run_state() != RunState::Working {
            return false;
        }
        if let Some(cancel) = &self.cancel {
            cancel.cancel();
        }
        self.dirty = true;
        true
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

    fn resolve_tool_confirmation(&mut self, allow: bool) {
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

    fn handle_tool_decision(
        &mut self,
        tool: &ToolCall,
        subject: &str,
        reply: tokio::sync::oneshot::Sender<ToolDecision>,
    ) {
        let args_json: serde_json::Value =
            serde_json::from_str(tool.arguments.as_str()).unwrap_or(serde_json::Value::Null);
        let Ok(args_lua) = self.inner.lua.to_value(&args_json) else {
            let _ = reply.send(ToolDecision::Deny {
                reason: String::from("failed to decode arguments"),
            });
            return;
        };
        let LuaValue::Table(args_table) = args_lua else {
            let _ = reply.send(ToolDecision::Deny {
                reason: String::from("arguments must be an object"),
            });
            return;
        };

        let Ok(event) = self.inner.lua.create_table() else {
            let _ = reply.send(ToolDecision::Deny {
                reason: String::from("failed to build event"),
            });
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

    fn stop_working(&mut self) {
        self.cancel = None;
        {
            let state_rc = self.inner.state();
            let mut state = state_rc.borrow_mut();
            state.set_run_state(RunState::Idle);
            state.set_turn_started(None);
        }
        self.inner.emit(events::STATUS_CHANGED, &[]);
    }

    fn maybe_submit_queued(&mut self) {
        if let Some(text) = self.queued.pop_front() {
            self.submit(&text);
        }
    }

    fn fail_assistant(&mut self, error: &str) {
        self.append(Message::Error {
            text: error.to_string(),
        });
    }

    fn finish_assistant(&mut self, text: &str, reasoning_content: Option<String>) {
        self.app.take_pending();
        self.append(Message::Assistant {
            text: text.to_string(),
            tool_calls: Vec::new(),
            reasoning_content,
        });
    }

    pub(crate) fn refresh_suggestions(&mut self) {
        let pool = suggest_pool(self);
        self.app.set_suggestions(pool);
    }

    pub(crate) fn on_timer(&mut self) {
        let working = self.inner.state().borrow().run_state() == RunState::Working;
        if working {
            self.inner.emit(events::TICK, &[]);
            self.dirty = true;
        }
    }

    pub(crate) fn timer_interval(&self) -> Duration {
        let ms = self.inner.state().borrow().opts().loader_interval_ms.max(1);
        Duration::from_millis(ms)
    }

    fn on_command(&mut self, command: &str) {
        let trimmed = command.trim();
        let (name, rest) = match trimmed.find(char::is_whitespace) {
            Some(i) => (&trimmed[..i], trimmed[i..].trim_start()),
            None => (trimmed, ""),
        };
        let args = Args::parse(rest);
        let lua_command = self.inner.api.commands().borrow().get(name).cloned();
        if let Some(mut action) = Builtin::from_name(name) {
            action.start(self, &args);
            self.active = Some(action);
        } else if let Some(handler) = lua_command {
            if let Err(err) = handler.call::<()>((args.raw.clone(),)) {
                self.inner.report(format!("{name}: {err}"));
            }
        } else {
            self.inner.report(format!("unknown command: {name}"));
        }
    }

    fn on_modal(&mut self, input: ModalInput) {
        let mut active = self.active.take();
        if let Some(action) = active.as_mut() {
            self.action_done = false;
            match input {
                ModalInput::Select(item) => action.on_select(self, item),
                ModalInput::Prompt(value) => action.on_prompt(self, value),
                ModalInput::Cancel => action.on_cancel(self),
            }
        }
        if let Some(action) = active
            && !self.action_done
        {
            self.active = Some(action);
        }
    }
}

impl Context for LoopData {
    fn open_select(&mut self, title: String, items: Vec<String>) {
        self.app.open_select(title, items);
    }

    fn open_prompt(&mut self, title: String, value: String, secret: bool) {
        self.app.open_prompt(title, value, secret);
    }

    fn set_setting(&mut self, key: &str, value: &str) {
        let _ = self.storage.set_setting(key, value);
    }

    fn get_setting(&mut self, key: &str) -> Option<String> {
        self.storage.get_setting(key).ok().flatten()
    }

    fn save_credential(&mut self, provider: &str, key: &str) {
        if let Err(err) = credential::set(provider, key) {
            self.inner
                .report(format!("failed to save credential: {err}"));
        }
    }

    fn resolve_llm(&mut self) {
        self.inner.resolve_llm(&mut *self.storage);
        self.inner.emit(events::STATUS_CHANGED, &[]);
        self.dirty = true;
    }

    fn reload(&mut self) {
        self.control = Control::Reload;
    }

    fn start_oauth(&mut self, provider_id: &str) {
        let Some(provider) = uji_core::llm::provider(provider_id) else {
            self.inner
                .report(format!("unknown provider: {provider_id}"));
            return;
        };
        super::auth::start(
            &self.runtime,
            Arc::clone(&self.inner.client),
            provider,
            self.background_tx.clone(),
        );
    }

    fn sync_packs(&mut self) {
        crate::pack::update_all(&self.inner.api);
        self.control = Control::Reload;
    }

    fn notify(&mut self, message: &str) {
        self.app.push_notices(vec![message.to_string()]);
        self.dirty = true;
    }

    fn finish(&mut self) {
        self.action_done = true;
    }

    fn command_names(&self) -> Vec<String> {
        suggest_pool(self)
            .into_iter()
            .map(|item| item.name)
            .collect()
    }
}

fn chord_of(key: KeyEvent) -> Option<Chord> {
    let mapped = match key.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Escape,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::Tab => Key::Tab,
        KeyCode::BackTab => Key::BackTab,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Insert => Key::Insert,
        KeyCode::F(number) => Key::F(number),
        _ => return None,
    };
    Some(Chord::new(
        mapped,
        key.modifiers.contains(KeyModifiers::CONTROL),
        key.modifiers.contains(KeyModifiers::ALT),
        key.modifiers.contains(KeyModifiers::SHIFT),
    ))
}

fn suggest_pool(data: &LoopData) -> Vec<SuggestItem> {
    let mut items: Vec<SuggestItem> = Builtin::ALL
        .iter()
        .map(|(name, desc)| SuggestItem {
            name: (*name).to_string(),
            desc: (*desc).to_string(),
        })
        .collect();
    let mut lua: Vec<SuggestItem> = data
        .inner
        .api
        .commands()
        .borrow()
        .keys()
        .map(|name| SuggestItem {
            name: name.clone(),
            desc: "lua command".into(),
        })
        .collect();
    items.append(&mut lua);
    items
}

fn parse_tool_decision(value: Option<LuaValue>) -> Option<ToolApproval> {
    let value = value?;
    let LuaValue::Table(table) = value else {
        return None;
    };
    if let Ok(Some(reason)) = table.get::<Option<String>>("deny") {
        return Some(ToolApproval::Deny { reason });
    }
    if let Ok(true) = table.get::<bool>("allow") {
        return Some(ToolApproval::Allow);
    }
    if let Ok(Some(title)) = table.get::<Option<String>>("ask") {
        return Some(ToolApproval::Ask { title: Some(title) });
    }
    if let Ok(true) = table.get::<bool>("ask") {
        return Some(ToolApproval::Ask { title: None });
    }
    None
}
