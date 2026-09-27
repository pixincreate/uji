# Installation

```sh
cargo install --git https://github.com/uji-labs/uji --locked uji
```

The build needs Rust 1.88 or newer, a C compiler and `make`. Cargo puts the
`uji` binary in `~/.cargo/bin`, and `uji --version` prints the version once
your shell finds it.

From a clone of the repository, run `cargo install --path crates/uji --locked`
in its top folder instead. Adding `--force` to either command replaces an
installed uji with a newer one.
