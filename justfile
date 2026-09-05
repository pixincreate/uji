default:
    @just --list

# Format check (no writes).
fmt:
    cargo fmt --all --check

# Apply formatting.
fmt-apply:
    cargo fmt --all

# Clippy with warnings denied, plus structural ast-grep rules.
# The binary front-end (src/bin/) is excluded from the print rules — printing
# is its job.
lint:
    cargo clippy --workspace --all-targets -- -D warnings
    ast-grep scan crates/core/src crates/tui/src crates/libuji/src/lib.rs crates/libuji/src/config.rs

# Structural rules only.
scan:
    ast-grep scan crates/core/src crates/tui/src crates/libuji/src/lib.rs crates/libuji/src/config.rs

# Unit + integration tests.
test:
    cargo test --workspace

# Everything CI would run.
check: fmt lint test
