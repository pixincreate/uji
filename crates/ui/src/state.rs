use std::time::{Duration, Instant};

use crate::config::{ConfirmConfig, ThemeConfig, UiConfig, overlay};
use crate::model::{
    ActiveModel, Builtin, Color, GlobalOpts, Line, ParseError, RunState, Size, WinOpts, WindowSpec,
};

#[derive(Debug, Default)]
pub struct UiState {
    windows: Vec<WindowSpec>,
    opts: GlobalOpts,
    active: ActiveModel,
    queued: Vec<String>,
    next_window_id: u32,
    run_state: RunState,
    turn_started: Option<Instant>,
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

    pub fn opts(&self) -> &GlobalOpts {
        &self.opts
    }

    pub fn show_thinking(&self) -> bool {
        self.opts.show_thinking
    }

    pub fn toggle_thinking(&mut self) -> bool {
        self.opts.show_thinking = !self.opts.show_thinking;
        self.opts.show_thinking
    }

    fn set_confirm(&mut self, config: &ConfirmConfig) {
        let confirm = &mut self.opts.confirm;
        overlay(&mut confirm.title, config.title.clone());
        overlay(&mut confirm.yes, config.yes.clone());
        overlay(&mut confirm.no, config.no.clone());
    }

    fn apply_theme(&mut self, config: &ThemeConfig) -> Result<(), ParseError> {
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
            overlay(slot, color(value)?);
        }
        let input = color(config.input.as_deref())?;
        let title = color(config.confirm_title.as_deref())?;
        let body = color(config.confirm_body.as_deref())?;
        let selected = color(config.confirm_selected.as_deref())?;
        let unselected = color(config.confirm_unselected.as_deref())?;
        self.opts.theme = theme;
        overlay(&mut self.opts.input_color, input.map(Some));
        let confirm = &mut self.opts.confirm;
        overlay(&mut confirm.title_color, title.map(Some));
        overlay(&mut confirm.body_color, body.map(Some));
        overlay(&mut confirm.selected, selected.map(Some));
        overlay(&mut confirm.unselected, unselected.map(Some));
        Ok(())
    }

    pub fn apply_config(&mut self, config: &UiConfig) -> Result<(), ParseError> {
        let loader = config.waiting.loader.as_ref();
        let interval = loader
            .and_then(|loader| loader.interval)
            .map(interval)
            .transpose()?;
        self.apply_theme(&config.theme)?;
        overlay(&mut self.opts.show_thinking, config.show_thinking);
        overlay(&mut self.opts.cursor_blink, config.input.cursor_blink);
        overlay(&mut self.opts.suggest_enabled, config.suggest.enabled);
        overlay(&mut self.opts.suggest_max_height, config.suggest.max_height);
        self.set_confirm(&config.confirm);
        overlay(
            &mut self.opts.loader_frames,
            loader.and_then(|loader| loader.frames.clone()),
        );
        overlay(&mut self.opts.loader_interval, interval);
        Ok(())
    }

    pub fn loader_frames(&self) -> &[String] {
        &self.opts.loader_frames
    }

    pub fn loader_interval(&self) -> Duration {
        self.opts.loader_interval
    }

    pub fn current_provider(&self) -> Option<&str> {
        self.active.provider.as_deref()
    }

    pub fn current_model(&self) -> Option<&str> {
        self.active.model.as_deref()
    }

    pub fn set_active(&mut self, active: ActiveModel) {
        self.active = active;
    }

    pub fn queued(&self) -> &[String] {
        &self.queued
    }

    pub fn set_queued(&mut self, queued: Vec<String>) {
        self.queued = queued;
    }

    pub fn context_window(&self) -> Option<u64> {
        self.active.context_window
    }

    pub fn current_effort(&self) -> Option<&str> {
        self.active.effort.as_deref()
    }

    pub fn run_state(&self) -> RunState {
        self.run_state
    }

    pub fn begin_work(&mut self) {
        self.run_state = RunState::Working;
        self.turn_started = Some(Instant::now());
    }

    pub fn end_work(&mut self) {
        self.run_state = RunState::Idle;
        self.turn_started = None;
    }

    pub fn turn_started(&self) -> Option<Instant> {
        self.turn_started
    }

    pub fn loader_frame(&self) -> String {
        let Some(started) = self.turn_started else {
            return String::new();
        };
        let frames = &self.opts.loader_frames;
        if frames.is_empty() {
            return String::new();
        }
        let interval = self.opts.loader_interval.as_millis().max(1);
        let len = u128::try_from(frames.len()).unwrap_or(1);
        let idx = (started.elapsed().as_millis() / interval) % len;
        let idx = usize::try_from(idx).unwrap_or(0);
        frames[idx].clone()
    }
}

fn color(value: Option<&str>) -> Result<Option<Color>, ParseError> {
    value.map(str::parse).transpose()
}

fn interval(seconds: f64) -> Result<Duration, ParseError> {
    Duration::try_from_secs_f64(seconds)
        .ok()
        .filter(|interval| !interval.is_zero())
        .ok_or_else(|| {
            ParseError(format!(
                "waiting.loader.interval must be a positive number of seconds, not {seconds}"
            ))
        })
}
