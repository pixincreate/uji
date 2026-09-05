//! Event names emitted by the harness (`PascalCase`, `nvim`-autocmd flavored).

/// A new session was created. Fields: `session_id`.
pub const SESSION_CREATED: &str = "SessionCreated";

/// An existing session was resumed. Fields: `session_id`.
pub const SESSION_RESUMED: &str = "SessionResumed";

/// Input was submitted. Fields: `text`.
pub const MESSAGE_SUBMITTED: &str = "MessageSubmitted";

/// A message was persisted. Fields: `type`, `text`.
pub const MESSAGE_APPENDED: &str = "MessageAppended";

/// The loop is about to exit.
pub const QUIT: &str = "Quit";
