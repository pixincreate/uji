//! Live UI state — the vim-object store the renderer projects every frame.
//!
//! This is the "editor state" counterpart from nvim: never a frozen snapshot.
//! It is mutated through the `uji` Lua API at boot (init.lua) and at runtime
//! (event handlers, `uji.schedule`), and the renderer re-projects it on every
//! frame.

use crate::model::{BufferKind, BufferSpec, GlobalOpts, UiModel, WinOpts, WindowSpec};

/// Live, mutable UI state: buffers and windows plus global options.
#[derive(Debug, Default)]
pub struct UiState {
    buffers: Vec<BufferSpec>,
    windows: Vec<WindowSpec>,
    opts: GlobalOpts,
}

impl UiState {
    /// Create an empty state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a buffer under `name` with the given content kind.
    pub fn push_buffer(&mut self, name: &str, kind: BufferKind) {
        self.buffers.push(BufferSpec {
            name: name.to_owned(),
            kind,
        });
    }

    /// Whether a buffer with this name is already registered.
    pub fn has_buffer(&self, name: &str) -> bool {
        self.buffers.iter().any(|b| b.name == name)
    }

    /// Register a window viewing `buffer`.
    pub fn push_window(&mut self, buffer: String, opts: WinOpts) {
        self.windows.push(WindowSpec { buffer, opts });
    }

    /// Remove every window viewing `buffer`, returning how many were removed.
    pub fn remove_windows_for(&mut self, buffer: &str) -> usize {
        let before = self.windows.len();
        self.windows.retain(|w| w.buffer != buffer);
        before - self.windows.len()
    }

    /// Remove all buffers and windows (used by config hot reload).
    pub fn clear(&mut self) {
        self.buffers.clear();
        self.windows.clear();
    }

    /// The registered buffers.
    pub fn buffers(&self) -> &[BufferSpec] {
        &self.buffers
    }

    /// The registered windows, in layout order.
    pub fn windows(&self) -> &[WindowSpec] {
        &self.windows
    }

    /// Global UI options.
    pub fn opts(&self) -> GlobalOpts {
        self.opts
    }

    /// Set whether the input cursor blinks.
    pub fn set_cursor_blink(&mut self, on: bool) {
        self.opts.cursor_blink = on;
    }

    /// Look up the content kind of a named buffer.
    pub fn buffer_kind(&self, name: &str) -> Option<BufferKind> {
        self.buffers.iter().find(|b| b.name == name).map(|b| b.kind)
    }

    /// Materialize the current state into a static [`UiModel`] (for
    /// embedders that want a snapshot).
    pub fn snapshot(&self) -> UiModel {
        UiModel {
            buffers: self.buffers.clone(),
            windows: self.windows.clone(),
            opts: self.opts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BufferKind, Size, Split};

    #[test]
    fn windows_and_buffers_accumulate_and_resolve() {
        let mut state = UiState::new();
        state.push_buffer("messages", BufferKind::Messages);
        state.push_buffer("input", BufferKind::Input);

        assert!(state.has_buffer("input"));
        assert_eq!(state.buffer_kind("messages"), Some(BufferKind::Messages));
        assert_eq!(state.buffer_kind("nope"), None);

        state.push_window(
            "input".into(),
            WinOpts {
                split: Split::Bottom,
                size: Size::Fixed(3),
                ..WinOpts::default()
            },
        );
        assert_eq!(state.windows().len(), 1);
        assert_eq!(state.windows()[0].buffer, "input");
    }
}
