use std::sync::Arc;
use std::time::Instant;

use uji_engine::llm::{AgentConfig, CancelToken, StreamEvent, run_agent};
use uji_engine::session::model::Message;
use uji_ui::model::RunState;

use super::LoopData;
use crate::runtime::events;
use crate::runtime::signal::Signal;

impl LoopData {
    pub(crate) fn submit(&mut self, text: &str) {
        if self.inner.state().borrow().run_state() == RunState::Working {
            self.enqueue(text);
            return;
        }
        if self.compact_if_needed() {
            self.enqueue(text);
            return;
        }
        self.app.overlay_mut().clear_notices();
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
            uji_engine::llm::context::build(conversation.messages())
        };
        let system = {
            let state_rc = self.inner.state();
            let state = state_rc.borrow();
            uji_engine::llm::system_prompt(
                state.opts().agent_system_prompt.as_deref(),
                &self.app.session().directory,
            )
        };
        let system = self.with_agent_context(system);
        let lua_tools = self.gather_lua_tools();
        let roots = {
            let access = self.inner.api.access().borrow();
            let extra = access.roots().to_vec();
            if access.confined() {
                uji_engine::tools::builtin::Roots::confined(extra)
            } else {
                uji_engine::tools::builtin::Roots::new(extra)
            }
        };
        let cwd = self.app.session().directory.clone();
        let sender = self.signals.clone();
        let cancel = CancelToken::new();
        self.cancel = Some(cancel.clone());
        let budget = self.budget();
        let keep_recent = self.keep_recent();
        let effort = *self.inner.llm_effort.borrow();
        let cache = *self.inner.llm_cache.borrow();
        let max_output = self.max_output();
        {
            let state_rc = self.inner.state();
            let mut state = state_rc.borrow_mut();
            state.set_run_state(RunState::Working);
            state.set_turn_started(Some(Instant::now()));
        }
        self.inner.emit(events::STATUS_CHANGED, &[]);
        self.runtime.spawn(async move {
            let tools = uji_engine::tools::builtin_registry(roots);
            let config = AgentConfig {
                client: &client,
                provider: provider.as_ref(),
                model,
                system: Some(system),
                tools: &tools,
                lua_tools: &lua_tools,
                cwd: std::path::Path::new(&cwd),
                cancel,
                budget,
                keep_recent,
                effort,
                max_output,
                cache,
            };
            let mut on_event = |event: StreamEvent| {
                let _ = sender.send(Signal::Llm(event));
            };
            run_agent(&config, context, &mut on_event).await;
        });
    }

    pub(crate) fn interrupt(&mut self) -> bool {
        let Some(cancel) = self.cancel.take() else {
            return false;
        };
        cancel.cancel();
        self.dirty = true;
        true
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

    pub(super) fn stop_working(&mut self) {
        self.cancel = None;
        {
            let state_rc = self.inner.state();
            let mut state = state_rc.borrow_mut();
            state.set_run_state(RunState::Idle);
            state.set_turn_started(None);
        }
        self.inner.emit(events::STATUS_CHANGED, &[]);
        self.inner.emit(events::TURN_FINISHED, &[]);
    }
}
