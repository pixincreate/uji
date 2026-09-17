use ratatui::buffer::Buffer;
use ratatui::style::Modifier;

use crate::app::App;
use crate::app::selection::Point;

pub fn overlay(buf: &mut Buffer, app: &App) {
    let width = usize::from(buf.area.width);
    if width == 0 {
        return;
    }
    let mut screen = app.overlay().screen().borrow_mut();
    screen.sync(usize::from(buf.area.height), |rows| {
        for (row, cells) in rows.iter_mut().zip(buf.content.chunks_exact(width)) {
            row.clear();
            row.reserve(width);
            for cell in cells {
                row.push_str(cell.symbol());
            }
        }
    });
    let Some(selection) = screen.selection() else {
        return;
    };
    for (y, cells) in buf.content.chunks_exact_mut(width).enumerate() {
        let Ok(y) = u16::try_from(y) else {
            break;
        };
        for (x, cell) in cells.iter_mut().enumerate() {
            let Ok(x) = u16::try_from(x) else {
                break;
            };
            if selection.contains(Point::new(x, y)) {
                cell.modifier.insert(Modifier::REVERSED);
            }
        }
    }
}
