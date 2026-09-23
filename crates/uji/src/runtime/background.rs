use std::sync::Arc;

use uji_core::llm::context::{self, Cut};
use uji_core::llm::discover::{self, Windows};
use uji_core::llm::{Llm, LlmConfig, Provider, Usage, summary, title};
use uji_core::session::model::{Message, StoredMessage};

use super::signal::Signal;
use super::work::Work;

pub(crate) enum CompactEvent {
    Ready {
        summary: String,
        files: Vec<String>,
        cut: Cut,
        usage: Option<Usage>,
    },
    Failed,
}

pub(crate) struct CompactRequest {
    pub(crate) client: Arc<reqwest::Client>,
    pub(crate) provider: Arc<Llm>,
    pub(crate) model: String,
    pub(crate) earlier: Vec<StoredMessage>,
    pub(crate) previous: Option<String>,
    pub(crate) carried: Vec<String>,
    pub(crate) cut: Cut,
}

pub(crate) fn compact(work: &Work, request: CompactRequest) {
    work.spawn(async move {
        let refs: Vec<&Message> = request.earlier.iter().map(|entry| &entry.message).collect();
        let mut files = context::files_touched(&refs);
        context::merge_files(&mut files, &request.carried);
        let done = summary::generate(
            &request.client,
            request.provider.as_ref(),
            request.model,
            &refs,
            request.previous.as_deref(),
        )
        .await;
        let event = match done {
            Some(done) => CompactEvent::Ready {
                summary: done.summary,
                files,
                cut: request.cut,
                usage: done.usage,
            },
            None => CompactEvent::Failed,
        };
        Signal::Compacted(event)
    });
}

pub(crate) enum TitleEvent {
    Ready { title: String, usage: Option<Usage> },
    Unavailable,
}

pub(crate) fn title(
    work: &Work,
    client: Arc<reqwest::Client>,
    provider: Arc<Llm>,
    model: String,
    first_message: String,
) {
    work.spawn(async move {
        let event = match title::generate(&client, provider, model, &first_message).await {
            Some(titled) => TitleEvent::Ready {
                title: titled.title,
                usage: titled.usage,
            },
            None => TitleEvent::Unavailable,
        };
        Signal::Title(event)
    });
}

pub(crate) struct ModelsEvent {
    pub(crate) provider: String,
    pub(crate) windows: Vec<Windows>,
}

pub(crate) fn model_windows(work: &Work, client: &Arc<reqwest::Client>, providers: Vec<Provider>) {
    for provider in providers {
        let client = Arc::clone(client);
        work.stream(|signals| async move {
            let key =
                LlmConfig::for_provider(provider.id.clone(), Some(&provider), None).resolve_key();
            let Some(windows) = discover::windows(&client, &provider, key.as_deref()).await else {
                return;
            };
            let _ = signals.send(Signal::Models(ModelsEvent {
                provider: provider.id,
                windows,
            }));
        });
    }
}
