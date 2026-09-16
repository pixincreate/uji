use mlua::Function;

/// State for the open picker: the callbacks it was given, and which query
/// generation is current so stale results can be dropped.
#[derive(Default)]
pub struct Pick {
    preview: Option<Function>,
    query: Option<Function>,
    token: u64,
}

impl Pick {
    pub fn open(&mut self, preview: Option<Function>, query: Option<Function>) {
        self.preview = preview;
        self.query = query;
    }

    pub fn preview(&self) -> Option<Function> {
        self.preview.clone()
    }

    pub fn query(&self) -> Option<Function> {
        self.query.clone()
    }

    /// Start a new generation; results carrying an older token are stale.
    pub fn next_token(&mut self) -> u64 {
        self.token = self.token.wrapping_add(1);
        self.token
    }

    pub fn token(&self) -> u64 {
        self.token
    }
}
