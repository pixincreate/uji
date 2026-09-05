use crate::model::{BufferKind, BufferSpec, GlobalOpts, UiModel, WinOpts, WindowSpec};

#[derive(Debug, Default)]
pub struct UiState {
    buffers: Vec<BufferSpec>,
    windows: Vec<WindowSpec>,
    opts: GlobalOpts,
}

impl UiState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_buffer(&mut self, name: &str, kind: BufferKind) {
        self.buffers.push(BufferSpec {
            name: name.to_owned(),
            kind,
        });
    }

    pub fn has_buffer(&self, name: &str) -> bool {
        self.buffers.iter().any(|b| b.name == name)
    }

    pub fn push_window(&mut self, buffer: String, opts: WinOpts) {
        self.windows.push(WindowSpec { buffer, opts });
    }

    pub fn remove_windows_for(&mut self, buffer: &str) -> usize {
        let before = self.windows.len();
        self.windows.retain(|w| w.buffer != buffer);
        before - self.windows.len()
    }

    pub fn clear(&mut self) {
        self.buffers.clear();
        self.windows.clear();
    }

    pub fn buffers(&self) -> &[BufferSpec] {
        &self.buffers
    }

    pub fn windows(&self) -> &[WindowSpec] {
        &self.windows
    }

    pub fn opts(&self) -> GlobalOpts {
        self.opts
    }

    pub fn set_cursor_blink(&mut self, on: bool) {
        self.opts.cursor_blink = on;
    }

    pub fn buffer_kind(&self, name: &str) -> Option<BufferKind> {
        self.buffers.iter().find(|b| b.name == name).map(|b| b.kind)
    }

    pub fn snapshot(&self) -> UiModel {
        UiModel {
            buffers: self.buffers.clone(),
            windows: self.windows.clone(),
            opts: self.opts,
        }
    }
}
