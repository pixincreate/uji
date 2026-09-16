#[derive(Debug, Default)]
pub struct Composer {
    text: String,
    cursor: usize,
    revision: u64,
    recall: Recall,
    pastes: crate::app::paste::Pastes,
}

#[derive(Debug, Default)]
enum Recall {
    #[default]
    Editing,
    Browsing {
        at: usize,
        draft: String,
    },
}

impl Composer {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Bumped on every edit, so a cache of the laid-out input knows when it
    /// has gone stale without comparing the text itself.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    fn edit(&mut self, change: impl FnOnce(&mut String, &mut usize)) {
        self.recall = Recall::Editing;
        self.revision = self.revision.wrapping_add(1);
        change(&mut self.text, &mut self.cursor);
    }

    pub fn insert(&mut self, c: char) {
        self.edit(|text, cursor| {
            text.insert(*cursor, c);
            *cursor = cursor.saturating_add(c.len_utf8());
        });
    }

    pub fn paste(&mut self, text: &str) {
        let cleaned = crate::app::paste::clean(text);
        if cleaned.is_empty() {
            return;
        }
        let inserted = self.pastes.stash(&cleaned);
        self.edit(move |text, cursor| {
            text.insert_str(*cursor, &inserted);
            *cursor = cursor.saturating_add(inserted.len());
        });
    }

    pub fn backspace(&mut self) {
        if self.drop_marker() {
            return;
        }
        self.edit(|text, cursor| {
            if *cursor > 0 {
                let prev = prev_boundary(text, *cursor);
                text.remove(prev);
                *cursor = prev;
            }
        });
    }

    fn drop_marker(&mut self) -> bool {
        let Some((id, width)) = self
            .text
            .get(..self.cursor)
            .and_then(|before| self.pastes.marker_ending_at(before))
        else {
            return false;
        };
        self.pastes.forget(id);
        self.edit(move |text, cursor| {
            let from = cursor.saturating_sub(width);
            text.replace_range(from..*cursor, "");
            *cursor = from;
        });
        true
    }

    pub fn clear(&mut self) {
        self.pastes.clear();
        self.edit(|text, cursor| {
            text.clear();
            *cursor = 0;
        });
    }

    pub fn set(&mut self, next: String) {
        self.edit(move |text, cursor| {
            *cursor = next.len();
            *text = next;
        });
    }

    pub fn take(&mut self) -> String {
        let mut taken = String::new();
        self.edit(|text, cursor| {
            taken = std::mem::take(text);
            *cursor = 0;
        });
        let expanded = self.pastes.expand(&taken);
        self.pastes.clear();
        expanded
    }

    pub fn left(&mut self) {
        self.cursor = prev_boundary(&self.text, self.cursor);
    }

    pub fn right(&mut self) {
        self.cursor = next_boundary(&self.text, self.cursor);
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.text.len();
    }

    pub fn recall_prev(&mut self, lookup: impl Fn(usize) -> Option<String>) -> bool {
        let at = match &self.recall {
            Recall::Editing => 0,
            Recall::Browsing { at, .. } => at.saturating_add(1),
        };
        let Some(text) = lookup(at) else {
            return false;
        };
        if let Recall::Browsing { at: browsing, .. } = &mut self.recall {
            *browsing = at;
        } else {
            let draft = std::mem::take(&mut self.text);
            self.recall = Recall::Browsing { at, draft };
        }
        self.place(text);
        true
    }

    pub fn recall_next(&mut self, lookup: impl Fn(usize) -> Option<String>) -> bool {
        let Recall::Browsing { at, .. } = &self.recall else {
            return false;
        };
        let at = *at;
        let Some(newer) = at.checked_sub(1) else {
            let Recall::Browsing { draft, .. } = std::mem::take(&mut self.recall) else {
                return false;
            };
            self.place(draft);
            return true;
        };
        let Some(text) = lookup(newer) else {
            return false;
        };
        if let Recall::Browsing { at, .. } = &mut self.recall {
            *at = newer;
        }
        self.place(text);
        true
    }

    fn place(&mut self, text: String) {
        self.cursor = text.len();
        self.text = text;
    }
}

fn prev_boundary(text: &str, index: usize) -> usize {
    if index == 0 {
        return 0;
    }
    let mut at = index - 1;
    while at > 0 && !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

fn next_boundary(text: &str, index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    let mut at = index + 1;
    while at < text.len() && !text.is_char_boundary(at) {
        at += 1;
    }
    at
}
