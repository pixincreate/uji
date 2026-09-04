//! Barebone action dispatch.
//!
//! This is the seam where a submitted input line turns into real work in the
//! harness (e.g. invoking an agent / model via `ai`). Right now it does nothing.

use crate::ai;

/// Handle a submitted input line.
///
/// Returns an optional response string to display back in the harness.
// TODO: dispatch to the actual harness engine.
pub fn run(_input: &str) -> Option<String> {
    ai::complete(_input)
}
