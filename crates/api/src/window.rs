//! Window operations.

use tui::model::{BufferKind, WinOpts};
use tui::state::UiState;

use super::ApiError;
use super::buffer;

/// Open a window viewing `buffer`, auto-creating the buffer when its name
/// matches a built-in kind.
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

/// Close every window viewing `buffer`; returns the number removed.
pub fn close(state: &mut UiState, buffer: &str) -> usize {
    state.remove_windows_for(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tui::model::{Border, Size, Split};

    #[test]
    fn open_auto_creates_builtin_buffers() {
        let mut state = UiState::new();
        open(&mut state, "messages", WinOpts::default()).expect("builtin kind");
        assert!(state.has_buffer("messages"));
        assert_eq!(state.windows().len(), 1);
    }

    #[test]
    fn open_rejects_unknown_buffers() {
        let mut state = UiState::new();
        assert!(open(&mut state, "nope", WinOpts::default()).is_err());
    }

    #[test]
    fn close_removes_matching_windows() {
        let mut state = UiState::new();
        open(&mut state, "input", WinOpts::default()).expect("builtin kind");
        open(
            &mut state,
            "input",
            WinOpts {
                split: Split::Bottom,
                size: Size::Fixed(3),
                border: Border::Plain,
                title: None,
            },
        )
        .expect("builtin kind");
        assert_eq!(state.windows().len(), 2);

        assert_eq!(close(&mut state, "input"), 2);
        assert!(state.windows().is_empty());
    }
}
