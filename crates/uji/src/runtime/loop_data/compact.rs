use std::sync::Arc;
use std::time::Instant;

use uji_core::llm::{Budget, context};
use uji_core::session::model::Message;
use uji_screen::model::RunState;

use super::LoopData;
use crate::runtime::background::{self, CompactEvent};
use crate::runtime::events;

const KEEP_FRACTION: u64 = 4;

impl LoopData {
    pub(super) fn compact_if_needed(&mut self) -> bool {
        let Some(budget) = self.budget() else {
            return false;
        };
        if !budget.overflows(self.used_tokens()) {
            return false;
        }
        if self.run_compaction(self.keep_recent()) {
            return true;
        }
        self.inner.report(format!(
            "context is over the {} token budget but the latest turn cannot be compacted",
            budget.usable()
        ));
        false
    }

    pub(super) fn keep_recent(&self) -> u64 {
        let room = self
            .budget()
            .map_or_else(|| self.used_tokens(), Budget::usable);
        room / KEEP_FRACTION
    }

    pub(super) fn run_compaction(&mut self, keep_recent: u64) -> bool {
        if self.inner.state().borrow().run_state() == RunState::Working {
            return false;
        }
        let Some(cut) = self.cut(keep_recent) else {
            return false;
        };
        let earlier = {
            let conversation = self.app.messages();
            conversation.messages()[cut.from..cut.compacted].to_vec()
        };
        {
            let state_rc = self.inner.state();
            let mut state = state_rc.borrow_mut();
            state.set_run_state(RunState::Working);
            state.set_turn_started(Some(Instant::now()));
        }
        self.inner.emit(events::STATUS_CHANGED, &[]);
        background::compact(
            &self.runtime,
            Arc::clone(&self.inner.client),
            self.inner.llm.borrow().clone(),
            self.inner.llm_model.borrow().clone(),
            earlier,
            cut,
            self.signals.clone(),
        );
        true
    }

    pub(super) fn on_compacted(&mut self, event: CompactEvent) {
        match event {
            CompactEvent::Ready {
                summary,
                cut,
                usage,
            } => {
                if let Some(usage) = usage {
                    self.app.conversation().borrow_mut().add_cost(usage);
                }
                self.append(Message::Compaction {
                    summary,
                    through: cut.through,
                });
                self.inner
                    .report(format!("compacted {} earlier messages", cut.span()));
                self.inner
                    .emit(events::COMPACTED, &[("count", cut.span().to_string())]);
            }
            CompactEvent::Failed => self
                .inner
                .report(String::from("could not compact; sending the full context")),
        }
        self.stop_working();
        self.drain_diagnostics();
        self.maybe_submit_queued();
        self.dirty = true;
    }

    fn cut(&self, keep_recent: u64) -> Option<context::Cut> {
        let conversation = self.app.messages();
        context::find_cut(conversation.messages(), keep_recent)
    }

    fn used_tokens(&self) -> u64 {
        let conversation = self.app.messages();
        let reported = conversation.last_input();
        if reported > 0 {
            reported
        } else {
            context::estimate_tokens(conversation.messages())
        }
    }

    fn budget(&self) -> Option<Budget> {
        let model = self.inner.llm_model.borrow().clone();
        let id = self.inner.llm_provider.borrow().clone();
        let catalog = self.inner.api.providers();
        let catalog = catalog.borrow();
        catalog
            .get(&id)
            .and_then(|provider| provider.budget(&model))
    }
}
