use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::widgets::{Block, Borders};
use uji_api::model::{Border, WindowSpec};

pub(crate) const USER_BG: Color = Color::Rgb(0x34, 0x35, 0x41);
pub(crate) const TEXT: Color = Color::Rgb(0xd4, 0xd4, 0xd4);
pub(crate) const MUTED: Color = Color::Rgb(0x80, 0x80, 0x80);
pub(crate) const SELECTED_BG: Color = Color::Rgb(0x3a, 0x3a, 0x4a);

pub(crate) fn accent_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn border_fade(d: u16) -> Style {
    let v = match d {
        0 => 0x1a,
        1 => 0x2c,
        2 => 0x3e,
        _ => 0x50,
    };
    Style::default().fg(Color::Rgb(v, v, v + 3))
}

pub(crate) fn block_for(win: &WindowSpec) -> Option<Block<'static>> {
    let mut block = match win.opts.border {
        Border::None => return None,
        Border::Plain => Block::default().borders(Borders::ALL),
        Border::Rounded => Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED),
    };
    if let Some(title) = &win.opts.title {
        block = block.title(title.clone());
    }
    Some(block)
}
