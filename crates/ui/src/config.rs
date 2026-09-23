use serde::Deserialize;

/// Overwrite `slot` only when the config supplied a value.
pub fn overlay<T>(slot: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *slot = value;
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
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
    pub input: Option<String>,
    pub confirm_title: Option<String>,
    pub confirm_body: Option<String>,
    pub confirm_selected: Option<String>,
    pub confirm_unselected: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct UiConfig {
    pub show_thinking: Option<bool>,
    pub theme: ThemeConfig,
    pub input: InputConfig,
    pub suggest: SuggestConfig,
    pub waiting: WaitingConfig,
    pub confirm: ConfirmConfig,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfirmConfig {
    pub title: Option<String>,
    pub yes: Option<String>,
    pub no: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct InputConfig {
    pub cursor_blink: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct SuggestConfig {
    pub enabled: Option<bool>,
    pub max_height: Option<u16>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct LoaderConfig {
    pub frames: Option<Vec<String>>,
    pub interval: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct WaitingConfig {
    pub loader: Option<LoaderConfig>,
}
