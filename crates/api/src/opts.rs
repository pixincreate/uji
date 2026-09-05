use tui::state::UiState;

pub fn cursor_blink(state: &UiState) -> bool {
    state.opts().cursor_blink
}

pub fn set_cursor_blink(state: &mut UiState, on: bool) {
    state.set_cursor_blink(on);
}
