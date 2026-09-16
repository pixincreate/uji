use super::{Point, Span};

const JOINERS: &[char] = &['/', '-', '_', '.'];

pub(super) fn row(lines: &[String], y: u16) -> Vec<char> {
    lines
        .get(usize::from(y))
        .map(|line| line.chars().collect())
        .unwrap_or_default()
}

pub(super) fn line_span(lines: &[String], y: u16) -> Span {
    Span {
        start: 0,
        end: u16::try_from(row(lines, y).len()).unwrap_or(u16::MAX),
    }
}

pub(super) fn word_span(lines: &[String], point: Point) -> Span {
    let chars = row(lines, point.y);
    let parts = segments(&chars);
    let Some(at) = parts
        .iter()
        .position(|part| point.x >= part.start && point.x < part.end)
    else {
        return Span::cell(point.x);
    };
    let mut span = Span {
        start: parts[at].start,
        end: parts[at].end,
    };
    let mut index = at;
    while index > 0 && joins(parts[index.saturating_sub(1)], parts[index]) {
        index = index.saturating_sub(1);
        span.start = parts[index].start;
    }
    let mut index = at;
    while index.saturating_add(1) < parts.len()
        && joins(parts[index], parts[index.saturating_add(1)])
    {
        index = index.saturating_add(1);
        span.end = parts[index].end;
    }
    span
}

#[derive(Clone, Copy)]
struct Segment {
    start: u16,
    end: u16,
    wordlike: bool,
    joiner: bool,
}

fn joins(left: Segment, right: Segment) -> bool {
    left.wordlike && right.wordlike && (left.joiner || right.joiner)
}

fn segments(chars: &[char]) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    let mut at = 0usize;
    while at < chars.len() {
        let start = at;
        if JOINERS.contains(&chars[at]) {
            at = at.saturating_add(1);
            out.push(segment(start, at, true, true));
            continue;
        }
        let wordlike = chars[at].is_alphanumeric();
        while at < chars.len()
            && chars[at].is_alphanumeric() == wordlike
            && !JOINERS.contains(&chars[at])
        {
            at = at.saturating_add(1);
        }
        out.push(segment(start, at, wordlike, false));
    }
    out
}

fn segment(start: usize, end: usize, wordlike: bool, joiner: bool) -> Segment {
    Segment {
        start: u16::try_from(start).unwrap_or(u16::MAX),
        end: u16::try_from(end).unwrap_or(u16::MAX),
        wordlike,
        joiner,
    }
}
