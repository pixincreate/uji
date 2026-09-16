use crate::model::{Float, Size, Split, WindowSpec};
use ratatui::layout::Rect;

fn is_vertical(split: Split) -> bool {
    matches!(split, Split::Top | Split::Bottom)
}

pub fn layout(area: Rect, windows: &[WindowSpec]) -> Vec<Rect> {
    let mut rects = Vec::with_capacity(windows.len());
    let mut remaining_area = area;

    for (index, win) in windows.iter().enumerate() {
        if let Some(float) = win.opts.float {
            rects.push(centered(area, float));
            continue;
        }
        let vertical = is_vertical(win.opts.split);
        let later = &windows[index + 1..];

        let reserved: u16 = later
            .iter()
            .filter(|w| w.opts.float.is_none())
            .filter(|w| is_vertical(w.opts.split) == vertical)
            .filter_map(|w| match w.effective_size() {
                Size::Fixed(n) => Some(n),
                Size::Fill | Size::Auto => None,
            })
            .sum();
        let later_fills = later
            .iter()
            .filter(|w| w.opts.float.is_none())
            .filter(|w| is_vertical(w.opts.split) == vertical && w.effective_size() == Size::Fill)
            .count();
        let fills = u16::try_from(later_fills)
            .unwrap_or(u16::MAX)
            .saturating_add(1);

        let avail = if vertical {
            remaining_area.height
        } else {
            remaining_area.width
        };
        let take = match win.effective_size() {
            Size::Fill | Size::Auto => avail.saturating_sub(reserved) / fills.max(1),
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

/// A float is placed against the whole frame, not the remaining strip, so it
/// overlays the layout instead of shrinking it.
fn centered(area: Rect, float: Float) -> Rect {
    let width = float.width.resolve(area.width).min(area.width);
    let height = float.height.resolve(area.height).min(area.height);
    Rect {
        x: area.x.saturating_add(area.width.saturating_sub(width) / 2),
        y: area
            .y
            .saturating_add(area.height.saturating_sub(height) / 2),
        width,
        height,
    }
}
