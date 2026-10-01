use std::process::ExitCode;

use uji_kernel::{Options, Sources, Terminal};

fn main() -> ExitCode {
    let outcome = uji_kernel::run(Options {
        sources: Sources::Embedded(uji_lua::FILES),
        entry: String::from("uji.boot"),
        args: std::env::args().collect(),
        terminal: Terminal::Real,
    });
    for error in &outcome.errors {
        eprintln!("uji: error: {error}");
    }
    ExitCode::from(outcome.code)
}
