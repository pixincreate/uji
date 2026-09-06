use tui::model::{WinOpts, WindowKind};
use tui::state::UiState;

pub fn open(state: &mut UiState, kind: WindowKind, lines: Vec<String>, opts: WinOpts) -> u32 {
    state.push_window(kind, lines, opts)
}

pub fn close(state: &mut UiState, id: u32) -> bool {
    state.close_window(id)
}
