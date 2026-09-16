use uji_core::llm::StreamEvent;
use uji_core::session::model::Message;

use super::LoopData;
use crate::runtime::events;

impl LoopData {
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
            StreamEvent::Compacted { summary, usage } => {
                let _ = summary;
                if let Some(usage) = usage {
                    self.app.conversation().borrow_mut().add_cost(usage);
                }
                self.inner.report(String::from(
                    "context filled up mid-turn; compacted to continue",
                ));
                self.dirty = true;
            }
            StreamEvent::Restarted { attempt, of, wait } => {
                self.app.take_pending();
                self.inner.report(format!(
                    "request failed, retrying in {}s ({attempt}/{of})",
                    wait.as_secs().max(1)
                ));
                self.drain_diagnostics();
                self.dirty = true;
            }
            StreamEvent::SteerRequest { reply } => {
                let next = self.queued.pop_front();
                if let Some(text) = next.clone() {
                    self.app.take_pending();
                    self.append(Message::User { text });
                    self.sync_queue();
                    self.dirty = true;
                }
                let _ = reply.send(next);
            }
            StreamEvent::AssistantStep {
                text,
                tool_calls,
                reasoning_content,
            } => {
                self.app.take_pending();
                self.persist_assistant_step(text, tool_calls, reasoning_content);
            }
            StreamEvent::ToolProgress { name, chunk, .. } => {
                self.app.overlay_mut().set_running(Some((name, chunk)));
                self.dirty = true;
            }
            StreamEvent::ToolResult {
                tool_call_id,
                name,
                content,
            } => {
                self.app.overlay_mut().set_running(None);
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
            }
            StreamEvent::Usage(usage) => {
                self.inner.api.session().add_usage(usage);
                self.inner.emit(events::STATUS_CHANGED, &[]);
            }
            StreamEvent::Cancelled => {
                self.app.take_pending();
                self.fail_assistant("interrupted");
                self.stop_working();
            }
            StreamEvent::Failed(err) => {
                self.app.take_pending();
                self.fail_assistant(&err);
                self.stop_working();
            }
        }
    }
}
