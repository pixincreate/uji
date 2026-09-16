#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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
    pub const ALL: [Self; 5] = [
        Self::Off,
        Self::Minimal,
        Self::Low,
        Self::Medium,
        Self::High,
    ];

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
        Self::ALL
            .into_iter()
            .find(|effort| effort.name() == name.to_lowercase())
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl std::fmt::Display for Effort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Retention {
    Off,
    #[default]
    Short,
    Long,
}

impl Retention {
    pub const ALL: [Self; 3] = [Self::Off, Self::Short, Self::Long];

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
        Self::ALL
            .into_iter()
            .find(|retention| retention.name() == name.to_lowercase())
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Short => "short",
            Self::Long => "long",
        }
    }
}

impl std::fmt::Display for Retention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
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
