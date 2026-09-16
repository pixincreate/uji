use std::sync::Arc;

use super::{DEFAULT_MAX_OUTPUT, Effort, Llm, LlmRequest, Protocol, Retention, Usage, silent};
use crate::session::model::Message;

const PROMPT: &str = "You name coding sessions. Read the user's first message and reply \
with a title of at most six words that says what the work is about. Reply with the title \
alone: no quotes, no trailing punctuation, no markdown, no preamble.";

const MAX_INPUT: usize = 2_000;
const MAX_TITLE: usize = 60;

pub struct Titled {
    pub title: String,
    pub usage: Option<Usage>,
}

pub async fn generate(
    client: &reqwest::Client,
    provider: Arc<Llm>,
    model: String,
    first_message: &str,
) -> Option<Titled> {
    let request = LlmRequest {
        model,
        system: Some(PROMPT.to_string()),
        messages: vec![Message::User {
            text: clip(first_message, MAX_INPUT),
        }],
        tools: Vec::new(),
        effort: Effort::Off,
        cache: Retention::Off,
        max_output: DEFAULT_MAX_OUTPUT,
    };
    let response = provider.call(client, &request, &mut silent()).await.ok()?;
    let title = sanitize(&response.text)?;
    Some(Titled {
        title,
        usage: response.usage,
    })
}

pub fn sanitize(raw: &str) -> Option<String> {
    let line = raw.lines().find(|line| !line.trim().is_empty())?;
    let trimmed = line
        .trim()
        .trim_start_matches(['#', '-', '*', ' '])
        .trim_matches(['"', '\'', '`', ' '])
        .trim_end_matches(['.', '!', '?'])
        .trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(clip(trimmed, MAX_TITLE))
}

fn clip(value: &str, max: usize) -> String {
    match value.char_indices().nth(max) {
        Some((index, _)) => value[..index].trim_end().to_string(),
        None => value.to_string(),
    }
}
