use std::fmt::Write as _;
use std::io::BufRead;

const TRUNCATED: &str = " …[line truncated]";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Line {
    Read { truncated: bool },
    Eof,
}

pub fn read<R: BufRead>(reader: &mut R, line: &mut String, max: usize) -> std::io::Result<Line> {
    line.clear();
    let mut started = false;
    let mut truncated = false;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(if started {
                Line::Read { truncated }
            } else {
                Line::Eof
            });
        }
        started = true;
        let ended = available.iter().position(|byte| *byte == b'\n');
        let chunk = ended.map_or(available, |at| &available[..at]);
        let room = max.saturating_sub(line.len());
        let take = chunk.len().min(room);
        let kept = String::from_utf8_lossy(&chunk[..take]).into_owned();
        let consumed = chunk.len().saturating_add(usize::from(ended.is_some()));
        truncated |= take < chunk.len();
        line.push_str(&kept);
        reader.consume(consumed);
        if ended.is_some() {
            return Ok(Line::Read { truncated });
        }
    }
}

pub struct Page {
    offset: usize,
    limit: usize,
    budget: usize,
    total: usize,
    shown: usize,
    full: bool,
    text: String,
}

impl Page {
    pub fn new(offset: usize, limit: usize, budget: usize) -> Self {
        Self {
            offset,
            limit,
            budget,
            total: 0,
            shown: 0,
            full: false,
            text: String::new(),
        }
    }

    pub fn push(&mut self, line: &str, truncated: bool) {
        self.total = self.total.saturating_add(1);
        if self.full || self.total < self.offset {
            return;
        }
        let before = self.text.len();
        let marker = if truncated { TRUNCATED } else { "" };
        let _ = writeln!(self.text, "{:>5}| {line}{marker}", self.total);
        if self.shown > 0 && self.text.len() > self.budget {
            self.text.truncate(before);
            self.full = true;
            return;
        }
        self.shown = self.shown.saturating_add(1);
        self.full = self.shown >= self.limit;
    }

    pub fn total(&self) -> usize {
        self.total
    }

    pub fn finish(mut self) -> String {
        let last = self.offset.saturating_add(self.shown).saturating_sub(1);
        if last < self.total {
            let _ = write!(
                self.text,
                "\n[showed lines {}-{last} of {}; continue with offset {}]",
                self.offset,
                self.total,
                last.saturating_add(1)
            );
        }
        self.text
    }
}
