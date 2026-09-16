use uji_engine::llm::{Budget, Provider};

use super::LoopData;

impl LoopData {
    fn with_provider<T>(&self, read: impl FnOnce(&Provider, &str) -> T) -> Option<T> {
        let model = self.inner.llm_model.borrow().clone();
        let id = self.inner.llm_provider.borrow().clone();
        let catalog = self.inner.api.providers();
        let catalog = catalog.borrow();
        catalog.get(&id).map(|provider| read(provider, &model))
    }

    pub(super) fn max_output(&self) -> u32 {
        self.with_provider(|provider, model| {
            provider
                .model(model)
                .and_then(|model| model.output)
                .and_then(|output| u32::try_from(output).ok())
        })
        .flatten()
        .unwrap_or(uji_engine::llm::DEFAULT_MAX_OUTPUT)
    }

    pub(super) fn budget(&self) -> Option<Budget> {
        let mut budget = self.with_provider(Provider::budget).flatten()?;
        if let Some(reserve) = self.compaction_reserve() {
            budget.reserve = reserve.min(budget.window);
        }
        Some(budget)
    }
}
