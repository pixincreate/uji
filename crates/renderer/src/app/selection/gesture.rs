use std::time::{Duration, Instant};

use super::words;
use super::{Grain, Point, Selection, Span};

const MULTI_CLICK_WINDOW: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy)]
struct Click {
    at: Instant,
    y: u16,
    word: Span,
    count: u8,
}

#[derive(Debug, Default)]
pub(super) struct Gesture {
    selection: Option<Selection>,
    grain: Grain,
    initial: Option<Span>,
    last: Option<Click>,
    dragging: bool,
}

impl Gesture {
    pub(super) fn selection(&self) -> Option<Selection> {
        self.selection
    }

    pub(super) fn press(&mut self, point: Point, lines: &[String], now: Instant) {
        let word = words::word_span(lines, point);
        self.grain = grain_for(self.count(point, word, now));
        self.dragging = false;
        let span = unit(self.grain, point, lines);
        self.initial = Some(span);
        let head = if self.grain == Grain::Char {
            span.start
        } else {
            span.end
        };
        self.selection = Some(Selection::new(
            Point::new(span.start, point.y),
            Point::new(head, point.y),
        ));
    }

    pub(super) fn drag(&mut self, point: Point, lines: &[String]) {
        let Some(selection) = self.selection.as_mut() else {
            return;
        };
        self.dragging = true;
        let Some(initial) = self.initial.filter(|_| self.grain != Grain::Char) else {
            selection.head = point;
            return;
        };
        let span = unit(self.grain, point, lines);
        let row = selection.anchor.y;
        if point.y < row || (point.y == row && span.end <= initial.start) {
            *selection = Selection::new(
                Point::new(initial.end, row),
                Point::new(span.start, point.y),
            );
        } else {
            *selection = Selection::new(
                Point::new(initial.start, row),
                Point::new(span.end, point.y),
            );
        }
    }

    pub(super) fn clear(&mut self) -> bool {
        self.initial = None;
        self.grain = Grain::Char;
        self.dragging = false;
        self.selection.take().is_some()
    }

    fn count(&mut self, point: Point, word: Span, now: Instant) -> u8 {
        let repeat = self.last.filter(|last| {
            last.y == point.y
                && last.word == word
                && now.duration_since(last.at) < MULTI_CLICK_WINDOW
        });
        let count = repeat.map_or(1, |last| last.count.wrapping_rem(3).saturating_add(1));
        self.last = Some(Click {
            at: now,
            y: point.y,
            word,
            count,
        });
        count
    }
}

fn grain_for(count: u8) -> Grain {
    match count {
        2 => Grain::Word,
        3 => Grain::Line,
        _ => Grain::Char,
    }
}

fn unit(grain: Grain, point: Point, lines: &[String]) -> Span {
    match grain {
        Grain::Char => Span::cell(point.x),
        Grain::Word => words::word_span(lines, point),
        Grain::Line => words::line_span(lines, point.y),
    }
}
