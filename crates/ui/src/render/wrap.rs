pub(crate) fn text(source: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return source.lines().map(str::to_string).collect();
    }
    let mut out = Vec::new();
    for line in source.lines() {
        wrap_line(line, width, &mut out);
    }
    out
}

fn wrap_line(line: &str, width: usize, out: &mut Vec<String>) {
    if line.chars().count() <= width {
        out.push(line.to_string());
        return;
    }
    let mut row = Row::new(width);
    for word in line.split_whitespace() {
        if word.chars().count() > width {
            row.flush(out);
            for letter in word.chars() {
                row.letter(letter, out);
            }
        } else {
            row.word(word, out);
        }
    }
    row.flush(out);
}

struct Row {
    width: usize,
    text: String,
}

impl Row {
    fn new(width: usize) -> Self {
        Self {
            width,
            text: String::new(),
        }
    }

    fn len(&self) -> usize {
        self.text.chars().count()
    }

    fn flush(&mut self, out: &mut Vec<String>) {
        if !self.text.is_empty() {
            out.push(std::mem::take(&mut self.text));
        }
    }

    fn word(&mut self, word: &str, out: &mut Vec<String>) {
        if self.len() + 1 + word.chars().count() > self.width {
            self.flush(out);
        } else if !self.text.is_empty() {
            self.text.push(' ');
        }
        self.text.push_str(word);
    }

    fn letter(&mut self, letter: char, out: &mut Vec<String>) {
        if self.len() == self.width {
            self.flush(out);
        }
        self.text.push(letter);
    }
}

pub(crate) fn ranges(chars: &[char], width: usize) -> Vec<(usize, usize)> {
    if width == 0 || chars.is_empty() {
        return vec![(0, chars.len())];
    }
    let mut rows = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let hard_end = (start + width).min(chars.len());
        if hard_end == chars.len() {
            rows.push((start, hard_end));
            break;
        }
        let end = (start..hard_end)
            .rev()
            .find(|index| chars[*index] == ' ')
            .map_or(hard_end, |index| index + 1);
        rows.push((start, end));
        start = end;
    }
    rows
}
