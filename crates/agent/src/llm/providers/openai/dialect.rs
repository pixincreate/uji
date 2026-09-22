use serde_json::{Map, Value, json};

use crate::llm::{Compat, Effort, MaxTokensField, ThinkingFormat};

#[derive(Clone, Copy)]
enum Knob {
    Effort,
    NestedEffort,
    Switch,
    Flag,
}

const THINKING: &[(ThinkingFormat, &[Knob])] = &[
    (ThinkingFormat::OpenAi, &[Knob::Effort]),
    (ThinkingFormat::OpenRouter, &[Knob::NestedEffort]),
    (ThinkingFormat::DeepSeek, &[Knob::Switch, Knob::Effort]),
    (ThinkingFormat::Zai, &[Knob::Switch]),
    (ThinkingFormat::Qwen, &[Knob::Flag, Knob::Effort]),
    (ThinkingFormat::None, &[]),
];

impl Knob {
    fn field(self, level: Option<&'static str>) -> Option<(String, Value)> {
        let on = level.is_some();
        match self {
            Self::Effort => level.map(|level| (String::from("reasoning_effort"), json!(level))),
            Self::NestedEffort => {
                level.map(|level| (String::from("reasoning"), json!({ "effort": level })))
            }
            Self::Switch => Some((
                String::from("thinking"),
                json!({ "type": if on { "enabled" } else { "disabled" } }),
            )),
            Self::Flag => Some((String::from("enable_thinking"), json!(on))),
        }
    }
}

pub(super) fn fields(compat: Compat, effort: Effort, limit: u32) -> Map<String, Value> {
    let level = effort.enabled().then(|| effort.name());
    let knobs = THINKING
        .iter()
        .find(|(format, _)| *format == compat.thinking)
        .map_or(&[][..], |(_, knobs)| knobs);
    let limit = (compat.max_tokens_field != MaxTokensField::None)
        .then(|| (String::from(compat.max_tokens_field.name()), json!(limit)));
    knobs
        .iter()
        .filter_map(|knob| knob.field(level))
        .chain(limit)
        .collect()
}
