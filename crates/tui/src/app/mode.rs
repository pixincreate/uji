#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestItem {
    pub name: String,
    pub desc: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Echo {
    Plain,
    Hidden,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Select {
        title: String,
        items: Vec<String>,
        query: String,
        cursor: usize,
    },
    Prompt {
        title: String,
        value: String,
        echo: Echo,
    },
    Suggest {
        items: Vec<SuggestItem>,
        cursor: usize,
    },
    Confirm {
        title: String,
        body: String,
        allow: bool,
    },
}
