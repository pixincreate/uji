mod gesture;
mod screen;
mod words;

pub use screen::Screen;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Point {
    pub y: u16,
    pub x: u16,
}

impl Point {
    pub fn new(x: u16, y: u16) -> Self {
        Self { y, x }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u16,
    pub end: u16,
}

impl Span {
    fn cell(at: u16) -> Self {
        Self {
            start: at,
            end: at.saturating_add(1),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Grain {
    #[default]
    Char,
    Word,
    Line,
}

#[derive(Debug, Clone, Copy)]
pub struct Selection {
    anchor: Point,
    head: Point,
}

impl Selection {
    fn new(anchor: Point, head: Point) -> Self {
        Self { anchor, head }
    }

    fn bounds(self) -> (Point, Point) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    pub fn contains(self, point: Point) -> bool {
        let (start, end) = self.bounds();
        if point.y < start.y || point.y > end.y {
            return false;
        }
        if point.y == start.y && point.x < start.x {
            return false;
        }
        !(point.y == end.y && point.x >= end.x)
    }

    pub fn text(self, lines: &[String]) -> String {
        let (start, end) = self.bounds();
        (start.y..=end.y)
            .map(|y| {
                let chars = words::row(lines, y);
                let from = if y == start.y {
                    usize::from(start.x)
                } else {
                    0
                };
                let to = if y == end.y {
                    usize::from(end.x).min(chars.len())
                } else {
                    chars.len()
                };
                let text: String = chars.get(from..to).unwrap_or_default().iter().collect();
                text.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
