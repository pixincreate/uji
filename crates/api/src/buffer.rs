//! Buffer operations.

use tui::model::BufferKind;
use tui::state::UiState;

/// Register a buffer with the given kind.
pub fn create(state: &mut UiState, name: &str, kind: BufferKind) {
    state.push_buffer(name, kind);
}
