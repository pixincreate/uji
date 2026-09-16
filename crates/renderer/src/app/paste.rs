use std::collections::BTreeMap;

const MAX_LINES: usize = 1;
const MAX_CHARS: usize = 80;

#[derive(Debug)]
struct Entry {
    marker: String,
    content: String,
}

#[derive(Debug, Default)]
pub struct Pastes {
    entries: BTreeMap<u32, Entry>,
    counter: u32,
}

impl Pastes {
    pub fn stash(&mut self, text: &str) -> String {
        let lines = text.lines().count();
        let chars = text.chars().count();
        if lines <= MAX_LINES && chars <= MAX_CHARS {
            return text.to_string();
        }
        self.counter = self.counter.saturating_add(1);
        let marker = if lines > MAX_LINES {
            format!("[paste #{} +{lines} lines]", self.counter)
        } else {
            format!("[paste #{} {chars} chars]", self.counter)
        };
        self.entries.insert(
            self.counter,
            Entry {
                marker: marker.clone(),
                content: text.to_string(),
            },
        );
        marker
    }

    pub fn expand(&self, text: &str) -> String {
        self.entries.values().fold(text.to_string(), |text, entry| {
            text.replace(&entry.marker, &entry.content)
        })
    }

    pub fn marker_ending_at(&self, text: &str) -> Option<(u32, usize)> {
        self.entries
            .iter()
            .find(|(_, entry)| text.ends_with(&entry.marker))
            .map(|(id, entry)| (*id, entry.marker.len()))
    }

    pub fn forget(&mut self, id: u32) {
        self.entries.remove(&id);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.counter = 0;
    }
}

pub fn clean(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\t', "    ")
        .chars()
        .filter(|ch| *ch == '\n' || !ch.is_control())
        .collect()
}
