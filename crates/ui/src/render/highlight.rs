use ratatui::buffer::Buffer;
use ratatui::style::Modifier;

use crate::app::App;
use crate::app::selection::Point;

pub fn overlay(buf: &mut Buffer, app: &App) {
    let mut screen = app.overlay().screen().borrow_mut();
    screen.set_lines(text(buf));
    let Some(selection) = screen.selection() else {
        return;
    };
    let area = buf.area;
    for y in 0..area.height {
        for x in 0..area.width {
            if selection.contains(Point::new(x, y)) {
                buf[(area.x.saturating_add(x), area.y.saturating_add(y))]
                    .modifier
                    .insert(Modifier::REVERSED);
            }
        }
    }
}

fn text(buf: &Buffer) -> Vec<String> {
    let area = buf.area;
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(area.x.saturating_add(x), area.y.saturating_add(y))].symbol())
                .collect()
        })
        .collect()
}
