use ratatui::style::{Color as TColor, Modifier, Style};
use ratatui::symbols;
use ratatui::widgets::{Block, Borders, Padding};
use uji_screen::model::{Border, Color, Style as ApiStyle, Theme, WindowSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub text: TColor,
    pub muted: TColor,
    pub code: TColor,
    pub accent: TColor,
    pub user_bg: TColor,
    pub selected_bg: TColor,
    pub cursor: TColor,
    pub error: TColor,
    pub notice: TColor,
    pub border: TColor,
}

impl Default for Palette {
    fn default() -> Self {
        Self::of(&Theme::default())
    }
}

impl Palette {
    pub fn of(theme: &Theme) -> Self {
        Self {
            text: color_of(theme.text),
            muted: color_of(theme.muted),
            code: color_of(theme.code),
            accent: color_of(theme.accent),
            user_bg: color_of(theme.user_bg),
            selected_bg: color_of(theme.selected_bg),
            cursor: color_of(theme.cursor),
            error: color_of(theme.error),
            notice: color_of(theme.notice),
            border: color_of(theme.border),
        }
    }

    pub(crate) fn accent_style(self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }
}

pub(crate) fn color_of(color: Color) -> TColor {
    match color {
        Color::Black => TColor::Black,
        Color::Red => TColor::Red,
        Color::Green => TColor::Green,
        Color::Yellow => TColor::Yellow,
        Color::Blue => TColor::Blue,
        Color::Magenta => TColor::Magenta,
        Color::Cyan => TColor::Cyan,
        Color::Gray => TColor::Gray,
        Color::DarkGray => TColor::DarkGray,
        Color::LightRed => TColor::LightRed,
        Color::LightGreen => TColor::LightGreen,
        Color::LightYellow => TColor::LightYellow,
        Color::LightBlue => TColor::LightBlue,
        Color::LightMagenta => TColor::LightMagenta,
        Color::LightCyan => TColor::LightCyan,
        Color::White => TColor::White,
        Color::Rgb(r, g, b) => TColor::Rgb(r, g, b),
    }
}

pub(crate) fn span_style(style: &ApiStyle) -> Style {
    let mut out = Style::default();
    if let Some(fg) = style.fg {
        out = out.fg(color_of(fg));
    }
    if let Some(bg) = style.bg {
        out = out.bg(color_of(bg));
    }
    if style.bold {
        out = out.add_modifier(Modifier::BOLD);
    }
    if style.italic {
        out = out.add_modifier(Modifier::ITALIC);
    }
    if style.underline {
        out = out.add_modifier(Modifier::UNDERLINED);
    }
    out
}

pub(crate) fn span_background(style: &ApiStyle) -> Option<TColor> {
    style.bg.map(color_of)
}

const FADE_SCALE: u32 = 1000;

pub(crate) fn border_fade(palette: Palette, d: u16) -> Style {
    let factor = match d {
        0 => 325,
        1 => 550,
        2 => 775,
        _ => FADE_SCALE,
    };
    Style::default().fg(dim(palette.border, factor))
}

fn dim(color: TColor, factor: u32) -> TColor {
    let TColor::Rgb(r, g, b) = color else {
        return color;
    };
    let scale = |channel: u8| {
        let value = u32::from(channel)
            .saturating_mul(factor)
            .saturating_div(FADE_SCALE);
        u8::try_from(value).unwrap_or(u8::MAX)
    };
    TColor::Rgb(scale(r), scale(g), scale(b))
}

pub(crate) fn vertical_chrome(win: &WindowSpec) -> u16 {
    let borders = u16::from(win.opts.border != Border::None).saturating_mul(2);
    borders.saturating_add(win.opts.padding.saturating_mul(2))
}

pub(crate) fn block_for(win: &WindowSpec, palette: Palette) -> Option<Block<'static>> {
    let borders = match win.opts.border {
        Border::None => Borders::NONE,
        Border::Plain | Border::Rounded => Borders::ALL,
        Border::Horizontal => Borders::TOP | Borders::BOTTOM,
    };
    if win.opts.border == Border::None && win.opts.padding == 0 {
        return None;
    }
    let mut block = match win.opts.border {
        Border::Rounded => Block::default()
            .borders(borders)
            .border_set(symbols::border::ROUNDED),
        _ => Block::default().borders(borders),
    };
    block = block.padding(Padding::vertical(win.opts.padding));
    let border_style = win.opts.border_color.map_or_else(
        || Style::default().fg(palette.muted),
        |color| Style::default().fg(color_of(color)),
    );
    block = block.style(border_style);
    if let Some(title) = &win.opts.title {
        block = block.title(title.clone());
    }
    Some(block)
}
