default:
    @just --list

# Install git hooks (one-time setup).
setup:
    git config core.hooksPath .githooks

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
    ast-grep scan crates/engine/src crates/ui/src crates/uji/src

# Structural rules only.
scan:
    ast-grep scan crates/engine/src crates/ui/src crates/uji/src

# Unit + integration tests.
test:
    cargo test --workspace

# Everything CI would run.
check: fmt lint test
