//! Barebone action dispatch.
//!
//! This is the seam where a submitted input line turns into real work in the
//! harness (e.g. invoking an agent / model via [`crate::ai`]).

use crate::ai;

/// Handle a submitted input line.
///
/// Returns an optional response string to display back in the harness.
pub fn run(input: &str) -> Option<String> {
    ai::complete(input)
}
