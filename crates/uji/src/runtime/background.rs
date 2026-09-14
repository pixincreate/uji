use std::sync::Arc;

use uji_core::llm::context::Cut;
use uji_core::llm::{Llm, Usage, summary, title};
use uji_core::session::model::StoredMessage;

use super::signal::Signal;

pub(crate) enum CompactEvent {
    Ready {
        summary: String,
        cut: Cut,
        usage: Option<Usage>,
    },
    Failed,
}

pub(crate) fn compact(
    runtime: &tokio::runtime::Runtime,
    client: Arc<reqwest::Client>,
    provider: Arc<dyn Llm>,
    model: String,
    earlier: Vec<StoredMessage>,
    cut: Cut,
    sender: calloop::channel::Sender<Signal>,
) {
    runtime.spawn(async move {
        let event = match summary::generate(&client, provider, model, &earlier).await {
            Some(done) => CompactEvent::Ready {
                summary: done.summary,
                cut,
                usage: done.usage,
            },
            None => CompactEvent::Failed,
        };
        let _ = sender.send(Signal::Compacted(event));
    });
}

pub(crate) enum TitleEvent {
    Ready { title: String, usage: Option<Usage> },
    Unavailable,
}

pub(crate) fn title(
    runtime: &tokio::runtime::Runtime,
    client: Arc<reqwest::Client>,
    provider: Arc<dyn Llm>,
    model: String,
    first_message: String,
    sender: calloop::channel::Sender<Signal>,
) {
    runtime.spawn(async move {
        let event = match title::generate(&client, provider, model, &first_message).await {
            Some(titled) => TitleEvent::Ready {
                title: titled.title,
                usage: titled.usage,
            },
            None => TitleEvent::Unavailable,
        };
        let _ = sender.send(Signal::Title(event));
    });
}
