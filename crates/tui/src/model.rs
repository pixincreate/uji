//! Declarative UI model — vim-style objects (buffers + windows).
//!
//! Rust defines the object model and the primitive buffer kinds; layouts are
//! composed externally (by the Lua config in `libuji`, or programmatically by
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
    /// Buffer name, used to reference it from windows.
    pub name: String,
    /// What the buffer renders.
    pub kind: BufferKind,
}

/// Window border style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Border {
    /// No border.
    #[default]
    None,
    /// Sharp single-line border.
    Plain,
    /// Rounded single-line border.
    Rounded,
}

/// Window size: fill the remaining space, or a fixed number of rows/cols.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    /// Take whatever space remains after fixed windows.
    Fill,
    /// Take exactly `n` rows (vertical splits) or columns (horizontal).
    Fixed(u16),
}

/// Which edge of the remaining area a window splits from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Split {
    /// Take rows from the top.
    #[default]
    Top,
    /// Take rows from the bottom.
    Bottom,
    /// Take columns from the left.
    Left,
    /// Take columns from the right.
    Right,
}

/// Window options (the `uji.open_win` opts table).
#[derive(Debug, Clone, PartialEq)]
pub struct WinOpts {
    /// Edge to split from.
    pub split: Split,
    /// Fill or fixed extent.
    pub size: Size,
    /// Border style.
    pub border: Border,
    /// Optional title rendered in the border.
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
    /// Name of the buffer this window views.
    pub buffer: String,
    /// Window options.
    pub opts: WinOpts,
}

/// Global UI options (the `uji.opt` table).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobalOpts {
    /// Whether the input cursor blinks.
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
    /// Declared buffers.
    pub buffers: Vec<BufferSpec>,
    /// Declared windows, in layout order.
    pub windows: Vec<WindowSpec>,
    /// Global options.
    pub opts: GlobalOpts,
}

impl Default for UiModel {
    /// Mirrors `libuji/config/default.lua` — messages fill the top, input
    /// box (3 rows, plain border) at the bottom.
    fn default() -> Self {
        Self {
            buffers: vec![
                BufferSpec {
                    name: "messages".into(),
                    kind: BufferKind::Messages,
                },
                BufferSpec {
                    name: "input".into(),
                    kind: BufferKind::Input,
                },
            ],
            windows: vec![
                WindowSpec {
                    buffer: "messages".into(),
                    opts: WinOpts {
                        split: Split::Top,
                        size: Size::Fill,
                        ..WinOpts::default()
                    },
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
    /// Look up the content kind of a named buffer.
    pub fn buffer_kind(&self, name: &str) -> Option<BufferKind> {
        self.buffers.iter().find(|b| b.name == name).map(|b| b.kind)
    }
}

fn is_vertical(split: Split) -> bool {
    matches!(split, Split::Top | Split::Bottom)
}

/// Split `area` among `windows` in declaration order.
///
/// Fixed sizes carve their span off the remaining rect; fill windows share
/// whatever is left after reserving space for later fixed windows on the same
/// axis.
pub fn layout(area: Rect, windows: &[WindowSpec]) -> Vec<Rect> {
    let mut rects = Vec::with_capacity(windows.len());
    let mut remaining_area = area;

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
        let later_fills = later
            .iter()
            .filter(|w| is_vertical(w.opts.split) == vertical && w.opts.size == Size::Fill)
            .count();
        let fills = u16::try_from(later_fills)
            .unwrap_or(u16::MAX)
            .saturating_add(1);

        let avail = if vertical {
            remaining_area.height
        } else {
            remaining_area.width
        };
        let take = match win.opts.size {
            Size::Fill => avail.saturating_sub(reserved) / fills.max(1),
            Size::Fixed(n) => n.min(avail),
        };

        let (window_area, rest) = carve(remaining_area, win.opts.split, take);
        rects.push(window_area);
        remaining_area = rest;
    }
    rects
}

fn carve(area: Rect, split: Split, take: u16) -> (Rect, Rect) {
    match split {
        Split::Top => {
            let window_area = Rect {
                height: take,
                ..area
            };
            let rest = Rect {
                y: area.y + take,
                height: area.height.saturating_sub(take),
                ..area
            };
            (window_area, rest)
        }
        Split::Bottom => {
            let window_area = Rect {
                y: area.y + area.height.saturating_sub(take),
                height: take,
                ..area
            };
            let rest = Rect {
                height: area.height.saturating_sub(take),
                ..area
            };
            (window_area, rest)
        }
        Split::Left => {
            let window_area = Rect {
                width: take,
                ..area
            };
            let rest = Rect {
                x: area.x + take,
                width: area.width.saturating_sub(take),
                ..area
            };
            (window_area, rest)
        }
        Split::Right => {
            let window_area = Rect {
                x: area.x + area.width.saturating_sub(take),
                width: take,
                ..area
            };
            let rest = Rect {
                width: area.width.saturating_sub(take),
                ..area
            };
            (window_area, rest)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(split: Split, size: Size) -> WindowSpec {
        WindowSpec {
            buffer: String::new(),
            opts: WinOpts {
                split,
                size,
                ..WinOpts::default()
            },
        }
    }

    fn area() -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        }
    }

    #[test]
    fn fill_reserves_space_for_later_fixed_window() {
        let rects = layout(
            area(),
            &[
                window(Split::Top, Size::Fill),
                window(Split::Bottom, Size::Fixed(3)),
            ],
        );
        assert_eq!(rects[0].height, 21);
        assert_eq!(rects[0].y, 0);
        assert_eq!(rects[1].height, 3);
        assert_eq!(rects[1].y, 21);
    }

    #[test]
    fn fixed_first_carves_from_top() {
        let rects = layout(
            area(),
            &[
                window(Split::Top, Size::Fixed(5)),
                window(Split::Top, Size::Fill),
            ],
        );
        assert_eq!(rects[0].height, 5);
        assert_eq!(rects[1].height, 19);
        assert_eq!(rects[1].y, 5);
    }

    #[test]
    fn horizontal_splits_use_width() {
        let rects = layout(
            area(),
            &[
                window(Split::Left, Size::Fixed(30)),
                window(Split::Right, Size::Fill),
            ],
        );
        assert_eq!(rects[0].width, 30);
        assert_eq!(rects[1].width, 50);
        assert_eq!(rects[1].x, 30);
    }

    #[test]
    fn fixed_window_is_clamped_to_available_space() {
        let rects = layout(area(), &[window(Split::Bottom, Size::Fixed(100))]);
        assert_eq!(rects[0].height, 24);
    }

    #[test]
    fn multiple_fills_share_the_remainder() {
        let rects = layout(
            area(),
            &[
                window(Split::Top, Size::Fill),
                window(Split::Top, Size::Fill),
                window(Split::Bottom, Size::Fixed(4)),
            ],
        );
        assert_eq!(rects[0].height, 10);
        assert_eq!(rects[1].height, 10);
        assert_eq!(rects[2].height, 4);
    }
}
