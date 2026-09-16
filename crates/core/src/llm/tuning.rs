use std::str::FromStr;

use strum::{Display, EnumString, IntoStaticStr, VariantArray};

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Display, EnumString, IntoStaticStr, VariantArray,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum Effort {
    #[default]
    Off,
    Minimal,
    Low,
    Medium,
    High,
}

pub const MIN_ANSWER_TOKENS: u32 = 1024;
pub const DEFAULT_MAX_OUTPUT: u32 = 8192;

impl Effort {
    pub fn budget(self) -> u32 {
        match self {
            Self::Off => 0,
            Self::Minimal => 1024,
            Self::Low => 2048,
            Self::Medium => 8192,
            Self::High => 16384,
        }
    }

    pub fn enabled(self) -> bool {
        self != Self::Off
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::from_str(name).ok()
    }

    pub fn name(self) -> &'static str {
        self.into()
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Display, EnumString, IntoStaticStr, VariantArray,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum Retention {
    Off,
    #[default]
    Short,
    Long,
}

impl Retention {
    pub fn enabled(self) -> bool {
        self != Self::Off
    }

    pub fn ttl(self) -> Option<&'static str> {
        match self {
            Self::Long => Some("1h"),
            Self::Off | Self::Short => None,
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::from_str(name).ok()
    }

    pub fn name(self) -> &'static str {
        self.into()
    }
}

pub fn fit_thinking(effort: Effort, max_output: u32) -> (u32, u32) {
    let max_tokens = max_output.max(MIN_ANSWER_TOKENS);
    let mut budget = effort.budget();
    if budget > 0 && max_tokens <= budget {
        budget = budget.min(max_tokens.saturating_sub(MIN_ANSWER_TOKENS));
    }
    (max_tokens, budget)
}
