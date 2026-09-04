//! Declarative UI model — vim-style objects (buffers + windows).
//!
//! Rust defines the object model and the primitive buffer kinds; layouts are
//! composed externally (by the Lua config in libuji, or programmatically by
//! embedders). Nothing here hardcodes where anything renders.

use ratatui::layout::Rect;

/// Content kind of a buffer — the primitive components the renderer knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferKind {
    /// Pi-style message history.
    Messages,
    /// Editable input line with cursor.
    Input,
}

/// A buffer object (`uji.create_buf`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferSpec {
    pub name: String,
    pub kind: BufferKind,
}

/// Window border style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Border {
    #[default]
    None,
    Plain,
    Rounded,
}

/// Window size: fill the remaining space, or a fixed number of rows/cols.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Fill,
    Fixed(u16),
}

/// Which edge of the remaining area a window splits from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Split {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

/// Window options (the `uji.open_win` opts table).
#[derive(Debug, Clone, PartialEq)]
pub struct WinOpts {
    pub split: Split,
    pub size: Size,
    pub border: Border,
    pub title: Option<String>,
}

impl Default for WinOpts {
    fn default() -> Self {
        Self {
            split: Split::Top,
            size: Size::Fill,
            border: Border::None,
            title: None,
        }
    }
}

/// A window object — a view of a buffer.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowSpec {
    pub buffer: String,
    pub opts: WinOpts,
}

/// Global UI options (the `uji.opt` table).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobalOpts {
    pub cursor_blink: bool,
}

impl Default for GlobalOpts {
    fn default() -> Self {
        Self { cursor_blink: true }
    }
}

/// The composed UI: buffers + windows, vim-objects style.
#[derive(Debug, Clone, PartialEq)]
pub struct UiModel {
    pub buffers: Vec<BufferSpec>,
    pub windows: Vec<WindowSpec>,
    pub opts: GlobalOpts,
}

impl Default for UiModel {
    /// Mirrors `libuji/config/default.lua` — messages fill the top, input
    /// box (3 rows, plain border) at the bottom.
    fn default() -> Self {
        Self {
            buffers: vec![
                BufferSpec { name: "messages".into(), kind: BufferKind::Messages },
                BufferSpec { name: "input".into(), kind: BufferKind::Input },
            ],
            windows: vec![
                WindowSpec {
                    buffer: "messages".into(),
                    opts: WinOpts { split: Split::Top, size: Size::Fill, border: Border::None, ..WinOpts::default() },
                },
                WindowSpec {
                    buffer: "input".into(),
                    opts: WinOpts {
                        split: Split::Bottom,
                        size: Size::Fixed(3),
                        border: Border::Plain,
                        ..WinOpts::default()
                    },
                },
            ],
            opts: GlobalOpts::default(),
        }
    }
}

impl UiModel {
    pub fn buffer_kind(&self, name: &str) -> Option<BufferKind> {
        self.buffers.iter().find(|b| b.name == name).map(|b| b.kind)
    }
}

fn is_vertical(split: Split) -> bool {
    matches!(split, Split::Top | Split::Bottom)
}

/// Split `area` among `windows` in declaration order. Fixed sizes carve
/// their span off the remaining rect; Fill windows share whatever is left
/// after reserving space for later fixed windows on the same axis.
pub fn layout(area: Rect, windows: &[WindowSpec]) -> Vec<Rect> {
    let mut rects = Vec::with_capacity(windows.len());
    let mut remaining = area;

    for (index, win) in windows.iter().enumerate() {
        let vertical = is_vertical(win.opts.split);
        let later = &windows[index + 1..];

        let reserved: u16 = later
            .iter()
            .filter(|w| is_vertical(w.opts.split) == vertical)
            .filter_map(|w| match w.opts.size {
                Size::Fixed(n) => Some(n),
                Size::Fill => None,
            })
            .sum();
        let fills = 1 + later
            .iter()
            .filter(|w| is_vertical(w.opts.split) == vertical && w.opts.size == Size::Fill)
            .count() as u16;

        let avail = if vertical { remaining.height } else { remaining.width };
        let take = match win.opts.size {
            Size::Fill => avail.saturating_sub(reserved) / fills.max(1),
            Size::Fixed(n) => n.min(avail),
        };

        let (rect, rest) = carve(remaining, win.opts.split, take);
        rects.push(rect);
        remaining = rest;
    }
    rects
}

fn carve(area: Rect, split: Split, take: u16) -> (Rect, Rect) {
    match split {
        Split::Top => {
            let rect = Rect { height: take, ..area };
            let rest = Rect { y: area.y + take, height: area.height.saturating_sub(take), ..area };
            (rect, rest)
        }
        Split::Bottom => {
            let rect = Rect { y: area.y + area.height.saturating_sub(take), height: take, ..area };
            let rest = Rect { height: area.height.saturating_sub(take), ..area };
            (rect, rest)
        }
        Split::Left => {
            let rect = Rect { width: take, ..area };
            let rest = Rect { x: area.x + take, width: area.width.saturating_sub(take), ..area };
            (rect, rest)
        }
        Split::Right => {
            let rect = Rect { x: area.x + area.width.saturating_sub(take), width: take, ..area };
            let rest = Rect { width: area.width.saturating_sub(take), ..area };
            (rect, rest)
        }
    }
}
