use std::collections::HashMap;
use std::io;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;
use uji_native::{Json, abi, native};

use super::Tty;
use crate::context;

const MODIFIERS: [(&str, Modifier); 7] = [
    ("bold", Modifier::BOLD),
    ("blink", Modifier::SLOW_BLINK),
    ("dim", Modifier::DIM),
    ("italic", Modifier::ITALIC),
    ("underline", Modifier::UNDERLINED),
    ("reverse", Modifier::REVERSED),
    ("strikethrough", Modifier::CROSSED_OUT),
];

#[derive(Clone, Copy)]
pub(crate) enum Shape {
    Block,
    Bar,
    Underline,
}

#[derive(Clone, Copy)]
pub(crate) struct Cursor {
    pub(crate) row: u16,
    pub(crate) col: u16,
    pub(crate) shape: Shape,
}

pub(crate) trait Surface {
    fn buffer(&mut self) -> &mut Buffer;
    fn size(&mut self) -> io::Result<(u16, u16)>;
    fn present(&mut self, cursor: Option<Cursor>) -> io::Result<()>;
    fn write(&mut self, bytes: &[u8]) -> io::Result<()>;
    fn suspend(&mut self) -> io::Result<()>;
    fn resume(&mut self) -> io::Result<()>;
    fn close(&mut self) -> io::Result<()>;
}

pub(crate) fn row_text(buffer: &Buffer, row: u16) -> String {
    let area = buffer.area;
    (area.left()..area.right())
        .map(|col| buffer[(col, row)].symbol())
        .collect()
}

pub(crate) struct Screen {
    surface: Box<dyn Surface>,
    styles: Vec<Style>,
    cursor: Option<Cursor>,
}

impl Screen {
    pub(crate) fn new(surface: Box<dyn Surface>) -> Self {
        Self {
            surface,
            styles: Vec::new(),
            cursor: None,
        }
    }

    fn resolve(&self, id: Option<usize>) -> io::Result<Style> {
        match id {
            None | Some(0) => Ok(Style::default()),
            Some(id) => self
                .styles
                .get(id.saturating_sub(1))
                .copied()
                .ok_or_else(|| io::Error::other(format!("unknown style {id}"))),
        }
    }

    fn put(&mut self, at: (u16, u16), right: u16, text: &str, style: Style) -> u16 {
        let (col, row) = at;
        if col >= right {
            return col;
        }
        let limit = usize::from(right.saturating_sub(col));
        self.surface
            .buffer()
            .set_stringn(col, row, text, limit, style)
            .0
    }

    fn cover(&mut self, area: Rect, style: Style, symbol: &str) {
        let buffer = self.surface.buffer();
        let area = area.intersection(buffer.area);
        for row in area.top()..area.bottom() {
            for col in area.left()..area.right() {
                if let Some(cell) = buffer.cell_mut((col, row)) {
                    cell.reset();
                    cell.set_symbol(symbol);
                    cell.set_style(style);
                }
            }
        }
    }
}

fn screen<T>(run: impl FnOnce(&mut Screen) -> io::Result<T>) -> io::Result<T> {
    context::with(|context| match &mut context.terminal {
        Some(Tty::Opened(screen, _)) => run(screen),
        _ => Err(io::Error::other("the terminal is not open")),
    })
    .unwrap_or_else(|| Err(io::Error::other("the kernel is not running")))
}

fn color(text: Option<String>) -> io::Result<Option<Color>> {
    text.map(|text| {
        text.parse::<Color>()
            .map_err(|_| io::Error::other(format!("invalid colour {text}")))
    })
    .transpose()
}

#[derive(Deserialize)]
struct StyleSpec {
    fg: Option<String>,
    bg: Option<String>,
    #[serde(flatten)]
    flags: HashMap<String, bool>,
}

impl StyleSpec {
    fn style(self) -> io::Result<Style> {
        let mut style = Style::default();
        if let Some(fg) = color(self.fg)? {
            style = style.fg(fg);
        }
        if let Some(bg) = color(self.bg)? {
            style = style.bg(bg);
        }
        Ok(MODIFIERS
            .into_iter()
            .filter(|(name, _)| self.flags.get(*name) == Some(&true))
            .fold(style, |style, (_, modifier)| style.add_modifier(modifier)))
    }
}

fn shape(name: Option<&str>) -> io::Result<Shape> {
    match name {
        None | Some("block") => Ok(Shape::Block),
        Some("bar") => Ok(Shape::Bar),
        Some("underline") => Ok(Shape::Underline),
        Some(other) => Err(io::Error::other(format!("unknown cursor shape {other}"))),
    }
}

#[native(class = screen, raise)]
fn size() -> io::Result<(u16, u16)> {
    screen(|screen| screen.surface.size())
}

#[native(class = screen, raise)]
fn style(spec: Json<StyleSpec>) -> io::Result<usize> {
    let style = spec.0.style()?;
    screen(|screen| {
        screen.styles.push(style);
        Ok(screen.styles.len())
    })
}

#[native(class = screen)]
fn put(row: u16, col: u16, stop: u16, text: &str, style: usize) -> i32 {
    let placed = screen(|screen| {
        let style = screen.resolve(Some(style))?;
        let area = screen.surface.buffer().area;
        if row >= area.height {
            return Ok(col);
        }
        Ok(screen.put((col, row), stop.min(area.width), text, style))
    });
    placed.map_or_else(
        |err| {
            abi::fail(&err);
            -1
        },
        i32::from,
    )
}

#[native(class = screen, raise)]
fn fill(
    row: u16,
    col: u16,
    width: u16,
    height: u16,
    style: Option<usize>,
    symbol: Option<&str>,
) -> io::Result<()> {
    screen(|screen| {
        let style = screen.resolve(style)?;
        screen.cover(
            Rect::new(col, row, width, height),
            style,
            symbol.unwrap_or(" "),
        );
        Ok(())
    })
}

#[native(class = screen, raise)]
fn paint(row: u16, col: u16, width: u16, height: u16, style: usize) -> io::Result<()> {
    screen(|screen| {
        let style = screen.resolve(Some(style))?;
        let buffer = screen.surface.buffer();
        let area = Rect::new(col, row, width, height).intersection(buffer.area);
        buffer.set_style(area, style);
        Ok(())
    })
}

#[native(class = screen)]
fn text(row: u16) -> Option<String> {
    screen(|screen| {
        let buffer = screen.surface.buffer();
        Ok((row < buffer.area.height).then(|| row_text(buffer, row)))
    })
    .ok()
    .flatten()
}

#[native(class = screen, raise)]
fn clear() -> io::Result<()> {
    screen(|screen| {
        screen.surface.buffer().reset();
        Ok(())
    })
}

#[native(class = screen, raise)]
fn cursor(row: Option<u16>, col: Option<u16>, name: Option<&str>) -> io::Result<()> {
    let cursor = match (row, col) {
        (Some(row), Some(col)) => Some(Cursor {
            row,
            col,
            shape: shape(name)?,
        }),
        _ => None,
    };
    screen(|screen| {
        screen.cursor = cursor;
        Ok(())
    })
}

#[native(class = screen, raise)]
fn flush() -> io::Result<()> {
    screen(|screen| screen.surface.present(screen.cursor))
}

#[native(class = screen, raise)]
fn write(bytes: &[u8]) -> io::Result<()> {
    screen(|screen| screen.surface.write(bytes))
}

#[native(class = screen, raise)]
fn suspend() -> io::Result<()> {
    screen(|screen| screen.surface.suspend())
}

#[native(class = screen, raise)]
fn resume() -> io::Result<()> {
    screen(|screen| screen.surface.resume())
}

#[native(class = screen, raise)]
fn close() -> io::Result<()> {
    screen(|screen| screen.surface.close())
}
