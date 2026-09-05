//! Global UI options.

use tui::state::UiState;

/// Whether the input cursor blinks.
pub fn cursor_blink(state: &UiState) -> bool {
    state.opts().cursor_blink
}

/// Set whether the input cursor blinks.
pub fn set_cursor_blink(state: &mut UiState, on: bool) {
    state.set_cursor_blink(on);
}
