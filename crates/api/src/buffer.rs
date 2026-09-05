use tui::model::BufferKind;
use tui::state::UiState;

pub fn create(state: &mut UiState, name: &str, kind: BufferKind) {
    state.push_buffer(name, kind);
}
