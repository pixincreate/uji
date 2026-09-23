use serde::Deserialize;

/// Overwrite `slot` only when the config supplied a value.
pub fn overlay<T>(slot: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *slot = value;
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CompactionConfig {
    pub enabled: Option<bool>,
    pub reserve: Option<u64>,
    pub keep_recent: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ThemeConfig {
    pub text: Option<String>,
    pub muted: Option<String>,
    pub code: Option<String>,
    pub accent: Option<String>,
    pub user_bg: Option<String>,
    pub selected_bg: Option<String>,
    pub cursor: Option<String>,
    pub error: Option<String>,
    pub notice: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct UiConfig {
    pub show_thinking: Option<bool>,
    pub theme: ThemeConfig,
    pub compaction: CompactionConfig,
    pub input: InputConfig,
    pub suggest: SuggestConfig,
    pub waiting: WaitingConfig,
    pub confirm: ConfirmConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfirmConfig {
    pub title: Option<String>,
    pub yes: Option<String>,
    pub no: Option<String>,
    pub selected: Option<String>,
    pub unselected: Option<String>,
    pub title_color: Option<String>,
    pub body_color: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct InputConfig {
    pub cursor_blink: Option<bool>,
    pub text_color: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SuggestConfig {
    pub enabled: Option<bool>,
    pub max_height: Option<u16>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct LoaderConfig {
    pub frames: Option<Vec<String>>,
    pub interval_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct WaitingConfig {
    pub loader: Option<LoaderConfig>,
}
