use std::time::Instant;

use crate::model::{
    Builtin, Color, ConfirmConfig, ConfirmOpts, GlobalOpts, Line, RunState, Size, ThemeConfig,
    UiConfig, WinOpts, WindowSpec,
};

#[derive(Debug, Default)]
pub struct UiState {
    windows: Vec<WindowSpec>,
    opts: GlobalOpts,
    current_provider: Option<String>,
    current_model: Option<String>,
    current_effort: Option<String>,
    context_window: Option<u64>,
    queued: Vec<String>,
    next_window_id: u32,
    run_state: RunState,
    turn_started: Option<Instant>,
    notices: Vec<String>,
}

impl UiState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_window(&mut self, builtin: Option<Builtin>, opts: WinOpts) -> u32 {
        let id = self.next_window_id;
        self.next_window_id += 1;
        let at = self
            .windows
            .iter()
            .position(|existing| existing.opts.priority > opts.priority)
            .unwrap_or(self.windows.len());
        self.windows.insert(
            at,
            WindowSpec {
                fitted: None,
                id,
                builtin,
                buffer: Vec::new(),
                opts,
            },
        );
        id
    }

    pub fn close_window(&mut self, id: u32) -> bool {
        let before = self.windows.len();
        self.windows.retain(|w| w.id != id);
        self.windows.len() != before
    }

    pub fn clear(&mut self) {
        self.windows.clear();
    }

    pub fn windows(&self) -> &[WindowSpec] {
        &self.windows
    }

    pub fn set_window_lines(&mut self, id: u32, lines: Vec<Line>) {
        if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
            window.buffer = lines;
        }
    }

    pub fn clear_window(&mut self, id: u32) {
        if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
            window.buffer.clear();
        }
    }

    pub fn set_window_fitted(&mut self, id: u32, rows: u16) -> bool {
        let Some(window) = self.windows.iter_mut().find(|window| window.id == id) else {
            return false;
        };
        if window.fitted == Some(rows) {
            return false;
        }
        window.fitted = Some(rows);
        true
    }

    pub fn set_window_size(&mut self, id: u32, size: Size) {
        if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
            window.opts.size = size;
        }
    }

    pub fn set_window_title(&mut self, id: u32, title: Option<String>) {
        if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
            window.opts.title = title;
        }
    }

    pub fn opts(&self) -> GlobalOpts {
        self.opts.clone()
    }

    pub fn set_cursor_blink(&mut self, on: bool) {
        self.opts.cursor_blink = on;
    }

    pub fn set_input_color(&mut self, color: Option<Color>) {
        self.opts.input_color = color;
    }

    pub fn set_suggest_enabled(&mut self, enabled: bool) {
        self.opts.suggest_enabled = enabled;
    }

    pub fn set_suggest_max_height(&mut self, max_height: u16) {
        self.opts.suggest_max_height = max_height;
    }

    pub fn set_agent_system_prompt(&mut self, prompt: Option<String>) {
        self.opts.agent_system_prompt = prompt;
    }

    pub fn set_confirm(&mut self, config: &ConfirmConfig) {
        let mut confirm = ConfirmOpts::default();
        if let Some(title) = &config.title {
            confirm.title.clone_from(title);
        }
        if let Some(yes) = &config.yes {
            confirm.yes.clone_from(yes);
        }
        if let Some(no) = &config.no {
            confirm.no.clone_from(no);
        }
        confirm.selected = parse_color(config.selected.as_deref(), &mut self.notices);
        confirm.unselected = parse_color(config.unselected.as_deref(), &mut self.notices);
        confirm.title_color = parse_color(config.title_color.as_deref(), &mut self.notices);
        confirm.body_color = parse_color(config.body_color.as_deref(), &mut self.notices);
        self.opts.confirm = confirm;
    }

    fn apply_theme(&mut self, config: &ThemeConfig) {
        let mut theme = self.opts.theme;
        for (slot, value) in [
            (&mut theme.text, config.text.as_deref()),
            (&mut theme.muted, config.muted.as_deref()),
            (&mut theme.code, config.code.as_deref()),
            (&mut theme.accent, config.accent.as_deref()),
            (&mut theme.user_bg, config.user_bg.as_deref()),
            (&mut theme.selected_bg, config.selected_bg.as_deref()),
            (&mut theme.cursor, config.cursor.as_deref()),
            (&mut theme.error, config.error.as_deref()),
            (&mut theme.notice, config.notice.as_deref()),
        ] {
            if let Some(color) = parse_color(value, &mut self.notices) {
                *slot = color;
            }
        }
        self.opts.theme = theme;
    }

    pub fn apply_config(&mut self, config: &UiConfig) {
        self.apply_theme(&config.theme);
        if let Some(enabled) = config.compaction.enabled {
            self.opts.compaction.enabled = enabled;
        }
        if let Some(reserve) = config.compaction.reserve {
            self.opts.compaction.reserve = Some(reserve);
        }
        if let Some(keep_recent) = config.compaction.keep_recent {
            self.opts.compaction.keep_recent = keep_recent;
        }
        if let Some(cursor_blink) = config.input.cursor_blink {
            self.set_cursor_blink(cursor_blink);
        }
        if let Some(color) = config.input.text_color.as_deref() {
            match color.parse::<Color>() {
                Ok(color) => self.set_input_color(Some(color)),
                Err(err) => self.notices.push(err.to_string()),
            }
        }
        if let Some(enabled) = config.suggest.enabled {
            self.set_suggest_enabled(enabled);
        }
        if let Some(max_height) = config.suggest.max_height {
            self.set_suggest_max_height(max_height);
        }
        if let Some(prompt) = config.agent.system_prompt.as_deref() {
            self.set_agent_system_prompt(Some(prompt.to_string()));
        }
        self.set_confirm(&config.confirm);
        if let Some(loader) = &config.waiting.loader {
            if let Some(frames) = &loader.frames {
                self.opts.loader_frames.clone_from(frames);
            }
            if let Some(interval_ms) = loader.interval_ms {
                self.opts.loader_interval_ms = interval_ms;
            }
        }
    }

    pub fn loader_frames(&self) -> &[String] {
        &self.opts.loader_frames
    }

    pub fn loader_interval_ms(&self) -> u64 {
        self.opts.loader_interval_ms
    }

    pub fn current_provider(&self) -> Option<&str> {
        self.current_provider.as_deref()
    }

    pub fn current_model(&self) -> Option<&str> {
        self.current_model.as_deref()
    }

    pub fn set_current_provider(&mut self, provider: String) {
        self.current_provider = Some(provider);
    }

    pub fn queued(&self) -> &[String] {
        &self.queued
    }

    pub fn set_queued(&mut self, queued: Vec<String>) {
        self.queued = queued;
    }

    pub fn context_window(&self) -> Option<u64> {
        self.context_window
    }

    pub fn set_context_window(&mut self, window: Option<u64>) {
        self.context_window = window;
    }

    pub fn current_effort(&self) -> Option<&str> {
        self.current_effort.as_deref()
    }

    pub fn set_current_effort(&mut self, effort: Option<String>) {
        self.current_effort = effort;
    }

    pub fn set_current_model(&mut self, model: String) {
        self.current_model = Some(model);
    }

    pub fn run_state(&self) -> RunState {
        self.run_state
    }

    pub fn set_run_state(&mut self, state: RunState) {
        self.run_state = state;
    }

    pub fn turn_started(&self) -> Option<Instant> {
        self.turn_started
    }

    pub fn set_turn_started(&mut self, started: Option<Instant>) {
        self.turn_started = started;
    }

    pub fn loader_frame(&self) -> String {
        let Some(started) = self.turn_started else {
            return String::new();
        };
        let frames = &self.opts.loader_frames;
        if frames.is_empty() {
            return String::new();
        }
        let interval = u128::from(self.opts.loader_interval_ms.max(1));
        let len = u128::try_from(frames.len()).unwrap_or(1);
        let idx = (started.elapsed().as_millis() / interval) % len;
        let idx = usize::try_from(idx).unwrap_or(0);
        frames[idx].clone()
    }

    pub fn notify(&mut self, message: String) {
        self.notices.push(message);
    }

    pub fn take_notices(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notices)
    }
}

fn parse_color(value: Option<&str>, notices: &mut Vec<String>) -> Option<Color> {
    let value = value?;
    match value.parse::<Color>() {
        Ok(color) => Some(color),
        Err(err) => {
            notices.push(err.to_string());
            None
        }
    }
}
