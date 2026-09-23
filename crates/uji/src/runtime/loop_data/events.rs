use mlua::Function;
use uji_agent::llm::Delta;

use super::LoopData;
use crate::api::agent::Report;
use crate::runtime::events;

pub(crate) struct Reported {
    pub(crate) event: Report,
    pub(crate) on_applied: Option<Function>,
}

impl LoopData {
    pub(crate) fn on_report(&mut self, reported: Reported) {
        if matches!(
            reported.event,
            Report::Cancelled | Report::Failed { .. } | Report::Restarted { .. }
        ) {
            self.app.reveal_all();
            self.release_deferred();
            self.apply_report(reported);
            return;
        }
        let waiting = self.app.revealing() && !reported.event.is_delta();
        if waiting || !self.deferred.is_empty() {
            self.deferred.push_back(reported);
            return;
        }
        self.apply_report(reported);
    }

    pub(super) fn apply_report(&mut self, reported: Reported) {
        self.apply_event(reported.event);
        if let Some(on_applied) = reported.on_applied
            && let Err(err) = on_applied.call::<()>(())
        {
            self.inner.report(format!("agent report: {err}"));
        }
    }

    fn apply_event(&mut self, event: Report) {
        match event {
            Report::Text { text } => {
                self.app.push_delta(Delta::Text(text));
                self.dirty = true;
            }
            Report::Reasoning { text } => {
                self.app.push_delta(Delta::Reasoning(text));
                self.dirty = true;
            }
            Report::Compacted { usage } => {
                if let Some(usage) = usage {
                    self.app.conversation().borrow_mut().add_cost(usage);
                }
                self.inner.report(String::from(
                    "context filled up mid-turn; compacted to continue",
                ));
                self.dirty = true;
            }
            Report::Restarted { attempt, of, wait } => {
                self.app.take_pending();
                self.inner.report(format!(
                    "request failed, retrying in {}s ({attempt}/{of})",
                    wait.max(1)
                ));
                self.drain_diagnostics();
                self.dirty = true;
            }
            Report::AssistantStep {
                text,
                tool_calls,
                reasoning_content,
            } => {
                self.app.take_pending();
                self.persist_assistant_step(text, tool_calls, reasoning_content);
            }
            Report::ToolResult {
                tool_call_id,
                name,
                content,
            } => {
                self.app.overlay_mut().set_running(None);
                self.persist_tool_result(tool_call_id, name, content);
            }
            Report::Done {
                text,
                reasoning_content,
            } => {
                self.finish_assistant(&text, reasoning_content);
                self.stop_working();
            }
            Report::Usage(usage) => {
                self.inner.api.session().add_usage(usage);
                self.inner.emit(events::Event::StatusChanged.name(), &[]);
            }
            Report::Cancelled => {
                self.app.take_pending();
                self.fail_assistant("interrupted");
                self.stop_working();
            }
            Report::Failed { message } => {
                self.app.take_pending();
                self.fail_assistant(&message);
                self.stop_working();
            }
        }
    }
}
