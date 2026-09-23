use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use serde_json::Value;
use uji_tests::{
    ALLOW_ALL, Request, SUBMIT, Sandbox, Server, Until, provider, text, tool_calls, tool_results,
};

struct Trip {
    sandbox: Sandbox,
    server: Server,
    results: Vec<String>,
}

fn counted(from: usize, to: usize) -> Vec<String> {
    (from..=to).map(|n| n.to_string()).collect()
}

fn round_trip(calls: &'static [(&'static str, &'static str)]) -> Result<Trip, Box<dyn Error>> {
    let round = AtomicUsize::new(0);
    let server = Server::start(move |request: &Request| {
        if request.has_tools() && round.fetch_add(1, Ordering::SeqCst) == 0 {
            tool_calls(0, calls)
        } else {
            text("done")
        }
    })?;
    let sandbox = Sandbox::new("tools")?;
    sandbox.file("notes.txt", "alpha\nline two\ngamma\ndelta\nepsilon\n")?;
    sandbox.file("big.txt", counted(1, 3000).join("\n") + "\n")?;
    sandbox.file("long.txt", "x".repeat(5000) + "\n")?;
    sandbox.file("empty.txt", "")?;
    sandbox.file("bin.dat", b"ab\0cd")?;
    std::fs::write(sandbox.root().join("outside.txt"), "outside")?;
    sandbox.config(&format!(
        "{}\n{ALLOW_ALL}\n{SUBMIT}",
        provider(&server.url, 100_000)
    ))?;
    let messages = sandbox.run(Until::TurnFinished, &[], Duration::from_secs(20))?;
    Ok(Trip {
        results: tool_results(&messages),
        sandbox,
        server,
    })
}

fn numbered<S: AsRef<str>>(lines: &[S], from: usize) -> String {
    lines
        .iter()
        .enumerate()
        .map(|(at, line)| format!("{:>5}| {}", at + from, line.as_ref()))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[test]
fn the_tool_specs_sent_to_the_model_match_the_rust_builtins() {
    let trip = round_trip(&[]).unwrap();
    let expected: Value = serde_json::from_str(include_str!("fixtures/tool_specs.json")).unwrap();
    assert_eq!(trip.server.turns()[0].body["tools"], expected);
}

#[test]
fn read_file_pages_numbered_lines_and_refuses_what_it_cannot_read() {
    let trip = round_trip(&[
        ("read_file", r#"{"path": "notes.txt"}"#),
        (
            "read_file",
            r#"{"path": "notes.txt", "offset": 3, "limit": 2}"#,
        ),
        ("read_file", r#"{"path": "notes.txt", "offset": 99}"#),
        ("read_file", r#"{"path": "."}"#),
        ("read_file", r#"{"path": "missing.txt"}"#),
        ("read_file", r#"{"path": "bin.dat"}"#),
        ("read_file", r#"{"path": "big.txt"}"#),
        ("read_file", r#"{"path": "big.txt", "offset": 2990}"#),
        ("read_file", r#"{"path": "long.txt"}"#),
        ("read_file", r#"{"path": "empty.txt"}"#),
        ("read_file", r#"{"path": "../outside.txt"}"#),
        ("read_file", r#"{"path": ""}"#),
    ])
    .unwrap();
    let notes = ["alpha", "line two", "gamma", "delta", "epsilon"];
    let long = format!("{} …[line truncated]", "x".repeat(2000));
    assert_eq!(
        trip.results,
        [
            numbered(&notes, 1),
            format!(
                "{}\n[showed lines 3-4 of 5; continue with offset 5]",
                numbered(&notes[2..4], 3)
            ),
            String::from("error: offset 99 is past the end of notes.txt (5 lines)"),
            String::from("error: . is a directory, not a file"),
            String::from("error: read missing.txt: No such file or directory (os error 2)"),
            String::from("error: bin.dat looks like a binary file"),
            format!(
                "{}\n[showed lines 1-2000 of 3000; continue with offset 2001]",
                numbered(&counted(1, 2000), 1)
            ),
            numbered(&counted(2990, 3000), 2990),
            numbered(&[&long], 1),
            String::from("empty.txt is empty"),
            format!(
                "error: ../outside.txt is outside the working directory ({}); tools can only reach files under it",
                trip.sandbox.work().display()
            ),
            String::from("error: `path` is required and must be a non-empty string"),
        ]
    );
}

#[test]
fn edit_file_replaces_exact_snippets_and_explains_what_it_could_not_do() {
    let trip = round_trip(&[
        (
            "edit_file",
            r#"{"path": "notes.txt", "old_string": "gamma", "new_string": "GAMMA"}"#,
        ),
        (
            "edit_file",
            r#"{"path": "notes.txt", "old_string": "line two", "new_string": "LINE TWO"}"#,
        ),
        (
            "edit_file",
            r#"{"path": "notes.txt", "old_string": "nope", "new_string": "x"}"#,
        ),
        (
            "edit_file",
            r#"{"path": "notes.txt", "old_string": "o", "new_string": "0"}"#,
        ),
        (
            "edit_file",
            r#"{"path": "notes.txt", "old_string": "e", "new_string": "E", "replace_all": true}"#,
        ),
        (
            "edit_file",
            r#"{"path": "notes.txt", "old_string": "same", "new_string": "same"}"#,
        ),
        ("edit_file", r#"{"path": "notes.txt", "new_string": "x"}"#),
        (
            "edit_file",
            r#"{"path": "missing.txt", "old_string": "a", "new_string": "b"}"#,
        ),
    ])
    .unwrap();
    assert_eq!(
        trip.results,
        [
            "edited notes.txt at line 3",
            "edited notes.txt at line 2",
            "error: old_string was not found in notes.txt. Read the file again and copy the snippet exactly, without line-number prefixes.",
            "edited notes.txt at line 5",
            "edited notes.txt: replaced 2 occurrences",
            "error: old_string and new_string are identical",
            "error: `old_string` is required and must be a non-empty string",
            "error: read missing.txt: No such file or directory (os error 2)",
        ]
    );
    let notes = std::fs::read_to_string(trip.sandbox.work().join("notes.txt")).unwrap();
    assert_eq!(notes, "alpha\nLINE TWO\nGAMMA\ndElta\nEpsil0n\n");
}

#[test]
fn write_file_creates_directories_and_reports_what_it_wrote() {
    let trip = round_trip(&[
        (
            "write_file",
            r#"{"path": "made/deep/new.txt", "content": "one\ntwo\n"}"#,
        ),
        (
            "write_file",
            r#"{"path": "notes.txt", "content": "replaced"}"#,
        ),
        ("write_file", r#"{"path": "blank.txt", "content": ""}"#),
    ])
    .unwrap();
    assert_eq!(
        trip.results,
        [
            "created made/deep/new.txt (2 lines)",
            "overwrote notes.txt (1 lines)",
            "created blank.txt (0 lines)",
        ]
    );
    let work = trip.sandbox.work();
    assert_eq!(
        std::fs::read_to_string(work.join("made/deep/new.txt")).unwrap(),
        "one\ntwo\n"
    );
    assert_eq!(
        std::fs::read_to_string(work.join("notes.txt")).unwrap(),
        "replaced"
    );
    assert_eq!(std::fs::read_to_string(work.join("blank.txt")).unwrap(), "");
}

#[test]
fn run_command_reports_output_exit_codes_and_timeouts() {
    let trip = round_trip(&[
        (
            "run_command",
            r#"{"command": "echo hi; sleep 0.1; echo err >&2"}"#,
        ),
        ("run_command", r#"{"command": "echo partial; exit 3"}"#),
        ("run_command", r#"{"command": "true"}"#),
        ("run_command", r#"{"command": "exit 4"}"#),
        ("run_command", r#"{"command": "seq 1 8000"}"#),
        ("run_command", r#"{"command": "pwd"}"#),
        ("run_command", r#"{"command": "sleep 5", "timeout": 1}"#),
        ("run_command", r#"{"command": ""}"#),
    ])
    .unwrap();
    let [both, partial, quiet, failed, long, pwd, slow, empty] = trip.results.as_slice() else {
        panic!("expected eight results, got {:?}", trip.results);
    };
    assert_eq!(both, "hi\nerr");
    assert_eq!(partial, "partial\n(exit code 3)");
    assert_eq!(quiet, "(no output, exit code 0)");
    assert_eq!(failed, "(exit code 4)");
    assert!(long.starts_with("… 3200 earlier lines dropped; full output in "));
    assert!(long.ends_with("\n7999\n8000"));
    let work = std::fs::canonicalize(trip.sandbox.work()).unwrap();
    assert_eq!(std::fs::canonicalize(pwd).unwrap(), work);
    assert_eq!(slow, "error: command timed out after 1s");
    assert_eq!(
        empty,
        "error: `command` is required and must be a non-empty string"
    );
}
