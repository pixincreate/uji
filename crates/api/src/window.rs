use tui::model::{BufferKind, WinOpts};
use tui::state::UiState;

use super::ApiError;
use super::buffer;

pub fn open(state: &mut UiState, buffer: &str, opts: WinOpts) -> Result<(), ApiError> {
    if !state.has_buffer(buffer) {
        let kind = buffer
            .parse::<BufferKind>()
            .ok()
            .ok_or_else(|| ApiError::UnknownBufferKind(buffer.to_owned()))?;
        buffer::create(state, buffer, kind);
    }
    state.push_window(buffer.to_owned(), opts);
    Ok(())
}

pub fn close(state: &mut UiState, buffer: &str) -> usize {
    state.remove_windows_for(buffer)
}
