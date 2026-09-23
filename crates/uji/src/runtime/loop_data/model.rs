use uji_core::llm::{Budget, Provider};

use super::LoopData;

impl LoopData {
    fn with_provider<T>(&self, read: impl FnOnce(&Provider, &str) -> T) -> Option<T> {
        let catalog = self.inner.api.providers().borrow();
        catalog
            .get(&self.inner.llm_provider)
            .map(|provider| read(provider, &self.inner.llm_model))
    }

    pub(super) fn max_output(&self) -> u32 {
        self.with_provider(|provider, model| {
            provider
                .model(model)
                .and_then(|model| model.output)
                .and_then(|output| u32::try_from(output).ok())
        })
        .flatten()
        .unwrap_or(uji_core::llm::DEFAULT_MAX_OUTPUT)
    }

    pub(super) fn budget(&self) -> Option<Budget> {
        let mut budget = self.with_provider(Provider::budget).flatten()?;
        if let Some(reserve) = self.compaction_reserve() {
            budget.reserve = reserve.min(budget.window);
        }
        Some(budget)
    }
}
