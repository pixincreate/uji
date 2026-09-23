use std::time::Instant;

use mlua::serde::SerializeOptions;
use mlua::{Function, LuaSerdeExt, Table};
use serde::Serialize;
use uji_agent::llm::ToolSpec;
use uji_agent::session::model::Message;
use uji_ui::model::RunState;

use super::LoopData;
use crate::runtime::events;

const LOOP: &str = "uji.loop";

#[derive(Serialize)]
struct Turn<'a> {
    text: &'a str,
    system: String,
    messages: Vec<Message>,
    tools: Vec<ToolSpec>,
    model: String,
    effort: &'static str,
    max_output: u32,
    cache: &'static str,
}

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
        self.inner.emit(
            events::Event::MessageSubmitted.name(),
            &[("text", text.to_string())],
        );

        self.append(Message::User {
            text: text.to_string(),
        });
        self.maybe_title(text);

        let mut messages = {
            let conversation = self.app.messages();
            uji_agent::llm::context::build(conversation.messages())
        };
        let system = self.prompt(text, &mut messages);
        let turn = Turn {
            text,
            system,
            messages,
            tools: self.gather_tools(),
            model: self.inner.llm_model.borrow().clone(),
            effort: self.inner.llm_effort.borrow().name(),
            max_output: self.max_output(),
            cache: self.inner.llm_cache.borrow().name(),
        };
        {
            let state_rc = self.inner.state();
            let mut state = state_rc.borrow_mut();
            state.set_run_state(RunState::Working);
            state.set_turn_started(Some(Instant::now()));
        }
        self.inner.emit(events::Event::StatusChanged.name(), &[]);
        match self.start_turn(&turn) {
            Ok(cancel) => self.turn = Some(cancel),
            Err(err) => {
                self.fail_assistant(&format!("{LOOP}: {err}"));
                self.stop_working();
            }
        }
    }

    fn start_turn(&self, turn: &Turn<'_>) -> mlua::Result<Function> {
        let options = SerializeOptions::new()
            .serialize_none_to_null(false)
            .serialize_unit_to_null(false);
        let turn = self.inner.lua.to_value_with(turn, options)?;
        self.inner
            .require::<Table>(LOOP)?
            .get::<Function>("start")?
            .call(turn)
    }

    pub(crate) fn interrupt(&mut self) -> bool {
        if self.cancel_shell() {
            return true;
        }
        let Some(turn) = self.turn.take() else {
            return false;
        };
        if let Err(err) = turn.call::<()>(()) {
            self.inner.report(format!("{LOOP}: {err}"));
        }
        self.dirty = true;
        true
    }

    pub(super) fn stop_working(&mut self) {
        self.turn = None;
        if let Some(awaiting) = self.awaiting.take() {
            self.release(awaiting);
        }
        {
            let state_rc = self.inner.state();
            let mut state = state_rc.borrow_mut();
            state.set_run_state(RunState::Idle);
            state.set_turn_started(None);
        }
        self.inner.emit(events::Event::StatusChanged.name(), &[]);
        self.inner.emit(events::Event::TurnFinished.name(), &[]);
    }
}
