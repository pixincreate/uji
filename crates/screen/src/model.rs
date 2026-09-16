use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    Messages,
    Input,
    Modal,
}

impl fmt::Display for Builtin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Messages => "messages",
            Self::Modal => "modal",
            Self::Input => "input",
        })
    }
}

impl FromStr for Builtin {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "messages" => Ok(Self::Messages),
            "input" => Ok(Self::Input),
            "modal" => Ok(Self::Modal),
            other => Err(ParseError(format!(
                "unknown view: {other} (expected \"messages\", \"input\" or \"modal\")"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Gray,
    DarkGray,
    LightRed,
    LightGreen,
    LightYellow,
    LightBlue,
    LightMagenta,
    LightCyan,
    White,
    Rgb(u8, u8, u8),
}

impl FromStr for Color {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(hex) = s.strip_prefix('#') {
            if hex.len() != 6 {
                return Err(ParseError(format!("invalid color: {s} (expected #rrggbb)")));
            }
            let parse = |i: usize| {
                u8::from_str_radix(&hex[i..i + 2], 16)
                    .map_err(|_| ParseError(format!("invalid color: {s}")))
            };
            return Ok(Self::Rgb(parse(0)?, parse(2)?, parse(4)?));
        }
        match s {
            "black" => Ok(Self::Black),
            "red" => Ok(Self::Red),
            "green" => Ok(Self::Green),
            "yellow" => Ok(Self::Yellow),
            "blue" => Ok(Self::Blue),
            "magenta" => Ok(Self::Magenta),
            "cyan" => Ok(Self::Cyan),
            "white" => Ok(Self::White),
            "gray" | "grey" => Ok(Self::Gray),
            "dark_gray" | "dark_grey" | "light_black" => Ok(Self::DarkGray),
            "light_red" => Ok(Self::LightRed),
            "light_green" => Ok(Self::LightGreen),
            "light_yellow" => Ok(Self::LightYellow),
            "light_blue" => Ok(Self::LightBlue),
            "light_magenta" => Ok(Self::LightMagenta),
            "light_cyan" => Ok(Self::LightCyan),
            other => Err(ParseError(format!("unknown color: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

impl Span {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: Style::default(),
        }
    }

    pub fn styled(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line {
    pub spans: Vec<Span>,
}

impl Line {
    pub fn blank() -> Self {
        Self { spans: Vec::new() }
    }

    pub fn single(span: Span) -> Self {
        Self { spans: vec![span] }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Border {
    #[default]
    None,
    Plain,
    Rounded,
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Fill,
    Fixed(u16),
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Split {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

pub const WIN_DEFAULT_PRIORITY: i64 = 50;

#[derive(Debug, Clone, PartialEq)]
pub struct WinOpts {
    pub priority: i64,
    pub split: Split,
    pub size: Size,
    pub border: Border,
    pub title: Option<String>,
    pub wrap: bool,
    pub border_color: Option<Color>,
    pub padding: u16,
}

impl Default for WinOpts {
    fn default() -> Self {
        Self {
            priority: WIN_DEFAULT_PRIORITY,
            split: Split::Top,
            size: Size::Fill,
            border: Border::None,
            title: None,
            wrap: false,
            border_color: None,
            padding: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowSpec {
    pub id: u32,
    pub builtin: Option<Builtin>,
    pub buffer: Vec<Line>,
    pub opts: WinOpts,
    pub fitted: Option<u16>,
}

impl WindowSpec {
    pub fn effective_size(&self) -> Size {
        match self.opts.size {
            Size::Auto => Size::Fixed(self.fitted.unwrap_or(0)),
            other => other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub text: Color,
    pub muted: Color,
    pub code: Color,
    pub accent: Color,
    pub user_bg: Color,
    pub selected_bg: Color,
    pub cursor: Color,
    pub error: Color,
    pub notice: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            text: Color::Rgb(0xd4, 0xd4, 0xd4),
            muted: Color::Rgb(0x80, 0x80, 0x80),
            code: Color::Rgb(0xe0, 0xaf, 0x68),
            accent: Color::Cyan,
            user_bg: Color::Rgb(0x34, 0x35, 0x41),
            selected_bg: Color::Rgb(0x3a, 0x3a, 0x4a),
            cursor: Color::White,
            error: Color::Red,
            notice: Color::Red,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactionOpts {
    pub enabled: bool,
    pub reserve: Option<u64>,
    pub keep_recent: u64,
}

impl Default for CompactionOpts {
    fn default() -> Self {
        Self {
            enabled: true,
            reserve: None,
            keep_recent: 20_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlobalOpts {
    pub cursor_blink: bool,
    pub input_color: Option<Color>,
    pub suggest_enabled: bool,
    pub suggest_max_height: u16,
    pub loader_frames: Vec<String>,
    pub loader_interval_ms: u64,
    pub agent_system_prompt: Option<String>,
    pub confirm: ConfirmOpts,
    pub theme: Theme,
    pub compaction: CompactionOpts,
}

impl Default for GlobalOpts {
    fn default() -> Self {
        Self {
            cursor_blink: true,
            input_color: None,
            suggest_enabled: true,
            suggest_max_height: 5,
            loader_frames: Vec::new(),
            loader_interval_ms: 80,
            agent_system_prompt: None,
            confirm: ConfirmOpts::default(),
            theme: Theme::default(),
            compaction: CompactionOpts::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfirmOpts {
    pub title: String,
    pub yes: String,
    pub no: String,
    pub selected: Option<Color>,
    pub unselected: Option<Color>,
    pub title_color: Option<Color>,
    pub body_color: Option<Color>,
}

impl Default for ConfirmOpts {
    fn default() -> Self {
        Self {
            title: String::from("Allow tool call?"),
            yes: String::from("Yes"),
            no: String::from("No"),
            selected: None,
            unselected: None,
            title_color: None,
            body_color: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunState {
    #[default]
    Idle,
    Working,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

impl FromStr for Split {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "top" => Ok(Self::Top),
            "bottom" => Ok(Self::Bottom),
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            other => Err(ParseError(format!("unknown split: {other}"))),
        }
    }
}

impl fmt::Display for Split {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::Right => "right",
        })
    }
}

impl FromStr for Border {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "plain" => Ok(Self::Plain),
            "rounded" => Ok(Self::Rounded),
            "horizontal" => Ok(Self::Horizontal),
            other => Err(ParseError(format!("unknown border: {other}"))),
        }
    }
}

impl fmt::Display for Border {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::None => "none",
            Self::Plain => "plain",
            Self::Rounded => "rounded",
            Self::Horizontal => "horizontal",
        })
    }
}

impl From<u16> for Size {
    fn from(n: u16) -> Self {
        Self::Fixed(n)
    }
}

impl FromStr for Size {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "fill" => Ok(Self::Fill),
            "auto" => Ok(Self::Auto),
            other => Err(ParseError(format!(
                "unknown size: {other} (expected \"fill\" or \"auto\")"
            ))),
        }
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fill => f.write_str("fill"),
            Self::Auto => f.write_str("auto"),
            Self::Fixed(n) => write!(f, "{n}"),
        }
    }
}
