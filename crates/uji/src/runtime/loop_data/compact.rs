use std::sync::Arc;
use std::time::Instant;

use uji_core::llm::{Budget, context};
use uji_core::session::model::Message;
use uji_screen::model::{CompactionOpts, RunState};

use super::LoopData;
use crate::runtime::background::{self, CompactEvent};
use crate::runtime::events;

const KEEP_CEILING_FRACTION: u64 = 4;

impl LoopData {
    pub(super) fn compact_if_needed(&mut self) -> bool {
        if !self.compaction_opts().enabled {
            return false;
        }
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
        self.compaction_opts()
            .keep_recent
            .min(room / KEEP_CEILING_FRACTION)
            .max(1)
    }

    fn compaction_opts(&self) -> CompactionOpts {
        self.inner.state().borrow().opts().compaction
    }

    pub(super) fn compaction_reserve(&self) -> Option<u64> {
        self.compaction_opts().reserve
    }

    pub(super) fn run_compaction(&mut self, keep_recent: u64) -> bool {
        if self.inner.state().borrow().run_state() == RunState::Working {
            return false;
        }
        let Some(cut) = self.cut(keep_recent) else {
            return false;
        };
        let (previous, carried, earlier) = {
            let conversation = self.app.messages();
            let span = &conversation.messages()[cut.from..cut.compacted];
            match span.first().map(|entry| &entry.message) {
                Some(Message::Compaction { summary, files, .. }) => {
                    (Some(summary.clone()), files.clone(), span[1..].to_vec())
                }
                _ => (None, Vec::new(), span.to_vec()),
            }
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
            background::CompactRequest {
                client: Arc::clone(&self.inner.client),
                provider: self.inner.llm.borrow().clone(),
                model: self.inner.llm_model.borrow().clone(),
                earlier,
                previous,
                carried,
                cut,
            },
            self.signals.clone(),
        );
        true
    }

    pub(super) fn on_compacted(&mut self, event: CompactEvent) {
        match event {
            CompactEvent::Ready {
                summary,
                files,
                cut,
                usage,
            } => {
                if let Some(usage) = usage {
                    self.app.conversation().borrow_mut().add_cost(usage);
                }
                self.append(Message::Compaction {
                    summary,
                    through: cut.through,
                    files,
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
        self.app.messages().used_tokens()
    }
}
