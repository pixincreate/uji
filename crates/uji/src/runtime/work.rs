use std::future::Future;

use super::signal::Signal;

pub(crate) type Signals = calloop::channel::Sender<Signal>;

pub(crate) struct Work {
    runtime: tokio::runtime::Runtime,
    signals: Signals,
}

impl Work {
    pub(crate) fn new(runtime: tokio::runtime::Runtime, signals: Signals) -> Self {
        Self { runtime, signals }
    }

    pub(crate) fn spawn(&self, task: impl Future<Output = Signal> + Send + 'static) {
        let signals = self.signals.clone();
        self.runtime.spawn(async move {
            let _ = signals.send(task.await);
        });
    }

    pub(crate) fn spawn_blocking(&self, task: impl FnOnce() -> Signal + Send + 'static) {
        let signals = self.signals.clone();
        self.runtime.spawn_blocking(move || {
            let _ = signals.send(task());
        });
    }

    pub(crate) fn stream<F>(&self, task: impl FnOnce(Signals) -> F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.runtime.spawn(task(self.signals.clone()));
    }
}
