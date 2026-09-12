use std::sync::Arc;

use uji_core::llm::{Llm, Usage, title};

use super::signal::Signal;

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
