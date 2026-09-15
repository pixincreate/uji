use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use serde_json::{Value, json};

use crate::llm::ToolSpec;

use super::Tool;

const MAX_READ_LINES: usize = 2_000;
const MAX_TOOL_OUTPUT: usize = 24_000;
const MAX_GREP_MATCHES: usize = 200;
const MAX_GREP_DEPTH: usize = 12;
const SKIP_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "build",
    "dist",
    ".venv",
    "vendor",
    "__pycache__",
];

fn schema(props: &Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": props,
        "required": required,
        "additionalProperties": false,
    })
}

/// Extra directories the tools may reach, beyond the working directory.
/// Reading a sibling repo is legitimate; reaching `~/.ssh` is not, so this is
/// opt-in and empty by default.
#[derive(Clone, Default)]
pub struct Roots {
    extra: Arc<Vec<PathBuf>>,
}

impl Roots {
    pub fn new(extra: Vec<PathBuf>) -> Self {
        Self {
            extra: Arc::new(extra),
        }
    }

    /// The working directory first, then any configured extras.
    fn candidates<'a>(&'a self, cwd: &'a Path) -> impl Iterator<Item = &'a Path> {
        std::iter::once(cwd).chain(self.extra.iter().map(PathBuf::as_path))
    }
}

/// A path confined to one root. Every operation goes through `dir`, which
/// cannot be escaped -- containment is the open itself, not a check performed
/// beforehand, so there is no window in which a symlink can be swapped in.
struct Confined {
    dir: Dir,
    rel: PathBuf,
}

/// Whether a relative path stays inside its root. Only used to pick a root and
/// to produce a message the model can act on; `Dir` is what enforces.
fn stays_within(path: &Path) -> bool {
    let mut depth = 0i32;
    for part in path.components() {
        match part {
            Component::ParentDir => depth -= 1,
            Component::Normal(_) => depth += 1,
            _ => {}
        }
        if depth < 0 {
            return false;
        }
    }
    true
}

/// Where `path` sits inside `root`, or `None` if it is not under it.
fn relative_to(root: &Path, path: &Path) -> Option<PathBuf> {
    let rel = if path.is_absolute() {
        path.strip_prefix(root).ok()?.to_path_buf()
    } else if stays_within(path) {
        path.to_path_buf()
    } else {
        return None;
    };
    Some(if rel.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        rel
    })
}

fn outside(path: &str, cwd: &Path) -> String {
    format!(
        "{path} is outside the working directory ({}); tools can only reach files under it",
        cwd.display()
    )
}

fn resolve(cwd: &Path, path: &str, roots: &Roots) -> Result<Confined, String> {
    let raw = Path::new(path);
    let (root, rel) = roots
        .candidates(cwd)
        .find_map(|root| relative_to(root, raw).map(|rel| (root, rel)))
        .ok_or_else(|| outside(path, cwd))?;
    let dir = Dir::open_ambient_dir(root, ambient_authority())
        .map_err(|err| format!("open {}: {err}", root.display()))?;
    Ok(Confined { dir, rel })
}

/// Translate a refusal by `Dir` into something the model can act on. Without
/// this a blocked symlink reads as a mysterious permission error.
fn fs_error(action: &str, path: &str, cwd: &Path, err: &std::io::Error) -> String {
    if err.kind() == std::io::ErrorKind::PermissionDenied {
        outside(path, cwd)
    } else {
        format!("{action} {path}: {err}")
    }
}

fn cap(text: String, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text;
    }
    let head: String = text.chars().take(max).collect();
    format!("{head}\n\n[output truncated: showed {max} of {count} characters]")
}

fn arg(args: &Value, key: &str) -> String {
    args.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn required(args: &Value, key: &str) -> Result<String, String> {
    let value = arg(args, key);
    if value.is_empty() {
        Err(format!(
            "`{key}` is required and must be a non-empty string"
        ))
    } else {
        Ok(value)
    }
}

fn usize_arg(args: &Value, key: &str) -> Option<usize> {
    args.get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn is_probably_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8_000).any(|byte| *byte == 0)
}

pub struct ReadFile {
    pub roots: Roots,
}

#[async_trait]
impl Tool for ReadFile {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read_file".into(),
            description: "Read a text file and return its contents with 1-based line numbers prefixed as `NNN| `. Read a file before editing it so `edit_file` snippets match exactly. Long files are returned in pages: use `offset` and `limit` to read further. The line numbers are display only - never include them in `edit_file` arguments.".into(),
            parameters: schema(
                &json!({
                    "path": {
                        "type": "string",
                        "description": "Path to the file, absolute or relative to the working directory.",
                    },
                    "offset": {
                        "type": "integer",
                        "description": "1-based line to start at. Defaults to 1.",
                        "minimum": 1,
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to return. Defaults to 2000.",
                        "minimum": 1,
                    },
                }),
                &["path"],
            ),
        }
    }

    fn subject(&self, args: &Value) -> String {
        arg(args, "path")
    }

    async fn run(&self, args: &Value, cwd: &Path) -> Result<String, String> {
        let path = required(args, "path")?;
        let target = resolve(cwd, &path, &self.roots)?;
        if target.dir.metadata(&target.rel).is_ok_and(|m| m.is_dir()) {
            return Err(format!("{path} is a directory; use list_dir"));
        }
        let bytes = target
            .dir
            .read(&target.rel)
            .map_err(|err| fs_error("read", &path, cwd, &err))?;
        if is_probably_binary(&bytes) {
            return Err(format!("{path} looks like a binary file"));
        }
        let text = String::from_utf8_lossy(&bytes);
        let total = text.lines().count();
        let offset = usize_arg(args, "offset").unwrap_or(1).max(1);
        let limit = usize_arg(args, "limit").unwrap_or(MAX_READ_LINES).max(1);
        if offset > total && total > 0 {
            return Err(format!(
                "offset {offset} is past the end of {path} ({total} lines)"
            ));
        }
        let mut out = String::new();
        for (index, line) in text.lines().enumerate().skip(offset - 1).take(limit) {
            let _ = writeln!(out, "{:>5}| {line}", index + 1);
        }
        if out.is_empty() {
            return Ok(format!("{path} is empty"));
        }
        let last = (offset + limit - 1).min(total);
        if last < total {
            let _ = write!(
                out,
                "\n[showed lines {offset}-{last} of {total}; call read_file again with offset {} for more]",
                last + 1
            );
        }
        Ok(cap(out, MAX_TOOL_OUTPUT))
    }
}

pub struct EditFile {
    pub roots: Roots,
}

#[async_trait]
impl Tool for EditFile {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "edit_file".into(),
            description: "Replace an exact snippet of an existing file, leaving the rest untouched. This is the tool to use for changing code. `old_string` must reproduce the file's current text byte for byte, including indentation and newlines, and must appear exactly once unless `replace_all` is true - include a few surrounding lines to make it unique. Do not include the `NNN| ` line-number prefixes that read_file adds. To delete code, pass an empty `new_string`.".into(),
            parameters: schema(
                &json!({
                    "path": {
                        "type": "string",
                        "description": "Path to the file to edit, absolute or relative to the working directory.",
                    },
                    "old_string": {
                        "type": "string",
                        "description": "Exact text to find, copied verbatim from the file.",
                    },
                    "new_string": {
                        "type": "string",
                        "description": "Text to put in its place. Empty string deletes the snippet.",
                    },
                    "replace_all": {
                        "type": "boolean",
                        "description": "Replace every occurrence instead of requiring exactly one. Defaults to false.",
                    },
                }),
                &["path", "old_string", "new_string"],
            ),
        }
    }

    fn subject(&self, args: &Value) -> String {
        arg(args, "path")
    }

    async fn run(&self, args: &Value, cwd: &Path) -> Result<String, String> {
        let path = required(args, "path")?;
        let old = required(args, "old_string")?;
        let new = arg(args, "new_string");
        let replace_all = args
            .get("replace_all")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if old == new {
            return Err("old_string and new_string are identical".into());
        }
        let target = resolve(cwd, &path, &self.roots)?;
        let text = target
            .dir
            .read_to_string(&target.rel)
            .map_err(|err| fs_error("read", &path, cwd, &err))?;
        let Some(at) = text.find(old.as_str()) else {
            return Err(format!(
                "old_string was not found in {path}. Read the file again and copy the snippet exactly, without line-number prefixes."
            ));
        };
        let count = text.matches(old.as_str()).count();
        if count > 1 && !replace_all {
            return Err(format!(
                "old_string matches {count} places in {path}. Add surrounding lines to make it unique, or pass replace_all: true."
            ));
        }
        let updated = if replace_all {
            text.replace(old.as_str(), &new)
        } else {
            text.replacen(old.as_str(), &new, 1)
        };
        target
            .dir
            .write(&target.rel, &updated)
            .map_err(|err| fs_error("write", &path, cwd, &err))?;
        if replace_all {
            Ok(format!("edited {path}: replaced {count} occurrences"))
        } else {
            Ok(format!(
                "edited {path} at line {}",
                text[..at].lines().count() + 1
            ))
        }
    }
}

pub struct WriteFile {
    pub roots: Roots,
}

#[async_trait]
impl Tool for WriteFile {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "write_file".into(),
            description: "Write a file from scratch, creating parent directories as needed. This replaces the entire file, so use it for new files only. To change an existing file use `edit_file` instead - overwriting loses everything you did not include.".into(),
            parameters: schema(
                &json!({
                    "path": {
                        "type": "string",
                        "description": "Path to write, absolute or relative to the working directory.",
                    },
                    "content": {
                        "type": "string",
                        "description": "Complete contents of the file.",
                    },
                }),
                &["path", "content"],
            ),
        }
    }

    fn subject(&self, args: &Value) -> String {
        arg(args, "path")
    }

    async fn run(&self, args: &Value, cwd: &Path) -> Result<String, String> {
        let path = required(args, "path")?;
        let content = arg(args, "content");
        let target = resolve(cwd, &path, &self.roots)?;
        let existed = target.dir.metadata(&target.rel).is_ok();
        if let Some(parent) = target.rel.parent().filter(|p| !p.as_os_str().is_empty()) {
            target
                .dir
                .create_dir_all(parent)
                .map_err(|err| fs_error("mkdir", &path, cwd, &err))?;
        }
        target
            .dir
            .write(&target.rel, &content)
            .map_err(|err| fs_error("write", &path, cwd, &err))?;
        let lines = content.lines().count();
        if existed {
            Ok(format!("overwrote {path} ({lines} lines)"))
        } else {
            Ok(format!("created {path} ({lines} lines)"))
        }
    }
}

pub struct ListDir {
    pub roots: Roots,
}

#[async_trait]
impl Tool for ListDir {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "list_dir".into(),
            description: "List the entries of a directory, one per line, marked `dir` or `file`. Use it to orient yourself in an unfamiliar tree. Skips version-control and build directories.".into(),
            parameters: schema(
                &json!({
                    "path": {
                        "type": "string",
                        "description": "Directory to list. Defaults to the working directory.",
                    },
                }),
                &[],
            ),
        }
    }

    fn subject(&self, args: &Value) -> String {
        let path = arg(args, "path");
        if path.is_empty() {
            String::from(".")
        } else {
            path
        }
    }

    async fn run(&self, args: &Value, cwd: &Path) -> Result<String, String> {
        let path = self.subject(args);
        let target = resolve(cwd, &path, &self.roots)?;
        let entries = target
            .dir
            .read_dir(&target.rel)
            .map_err(|err| fs_error("list", &path, cwd, &err))?;
        let mut rows: Vec<String> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
            if is_dir && SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            rows.push(format!("{} {name}", if is_dir { "dir " } else { "file" }));
        }
        if rows.is_empty() {
            return Ok(format!("{path} is empty"));
        }
        rows.sort();
        Ok(cap(rows.join("\n"), MAX_TOOL_OUTPUT))
    }
}

pub struct Grep {
    pub roots: Roots,
}

#[async_trait]
impl Tool for Grep {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "grep".into(),
            description: "Search text files for a regular expression and return matching lines as `path:line: text`. Use it to locate symbols, callers, and definitions before reading whole files. Skips binaries, version-control and build directories. Returns at most 200 matches; narrow the pattern or path if you hit the cap.".into(),
            parameters: schema(
                &json!({
                    "pattern": {
                        "type": "string",
                        "description": "Rust regular expression, for example `fn run_agent` or `impl \\w+ for OpenAi`.",
                    },
                    "path": {
                        "type": "string",
                        "description": "File or directory to search. Defaults to the working directory.",
                    },
                    "case_sensitive": {
                        "type": "boolean",
                        "description": "Match case exactly. Defaults to true.",
                    },
                }),
                &["pattern"],
            ),
        }
    }

    fn subject(&self, args: &Value) -> String {
        let path = arg(args, "path");
        if path.is_empty() {
            String::from(".")
        } else {
            path
        }
    }

    async fn run(&self, args: &Value, cwd: &Path) -> Result<String, String> {
        let pattern = required(args, "pattern")?;
        let case_sensitive = args
            .get("case_sensitive")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let regex = regex::RegexBuilder::new(&pattern)
            .case_insensitive(!case_sensitive)
            .build()
            .map_err(|err| format!("invalid pattern: {err}"))?;
        let path = self.subject(args);
        let target = resolve(cwd, &path, &self.roots)?;
        let mut matches = Vec::new();
        let whole_root = target.rel == Path::new(".");
        if target
            .dir
            .metadata(&target.rel)
            .is_ok_and(|meta| meta.is_file())
        {
            let label = target.rel.display().to_string();
            grep_file(&target.dir, &target.rel, &label, &regex, &mut matches);
        } else {
            let nested = if whole_root {
                None
            } else {
                Some(
                    target
                        .dir
                        .open_dir(&target.rel)
                        .map_err(|err| fs_error("search", &path, cwd, &err))?,
                )
            };
            let dir = nested.as_ref().unwrap_or(&target.dir);
            let prefix = if whole_root {
                PathBuf::new()
            } else {
                target.rel.clone()
            };
            walk_grep(dir, &prefix, &regex, 0, &mut matches);
        }
        if matches.is_empty() {
            return Ok(format!("no matches for `{pattern}`"));
        }
        let capped = matches.len() >= MAX_GREP_MATCHES;
        let mut out = matches.join("\n");
        if capped {
            let _ = write!(
                out,
                "\n\n[stopped at {MAX_GREP_MATCHES} matches; narrow the pattern or path]"
            );
        }
        Ok(cap(out, MAX_TOOL_OUTPUT))
    }
}

fn grep_file(dir: &Dir, rel: &Path, label: &str, regex: &regex::Regex, out: &mut Vec<String>) {
    let Ok(bytes) = dir.read(rel) else {
        return;
    };
    if is_probably_binary(&bytes) {
        return;
    }
    let text = String::from_utf8_lossy(&bytes);
    for (index, line) in text.lines().enumerate() {
        if out.len() >= MAX_GREP_MATCHES {
            return;
        }
        if regex.is_match(line) {
            let line = line.trim_end();
            let line: String = line.chars().take(400).collect();
            out.push(format!("{label}:{}: {line}", index + 1));
        }
    }
}

/// Recurses through `Dir` handles rather than reopening by path, so a
/// symlinked directory cannot walk the search out of the working directory.
fn walk_grep(dir: &Dir, prefix: &Path, regex: &regex::Regex, depth: usize, out: &mut Vec<String>) {
    if depth > MAX_GREP_DEPTH || out.len() >= MAX_GREP_MATCHES {
        return;
    }
    let Ok(entries) = dir.entries() else {
        return;
    };
    let mut items: Vec<(String, bool)> = entries
        .flatten()
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
            (name, is_dir)
        })
        .collect();
    items.sort();
    for (name, is_dir) in items {
        if out.len() >= MAX_GREP_MATCHES {
            return;
        }
        let shown = prefix.join(&name);
        if is_dir {
            if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            if let Ok(nested) = dir.open_dir(&name) {
                walk_grep(&nested, &shown, regex, depth.saturating_add(1), out);
            }
        } else {
            grep_file(
                dir,
                Path::new(&name),
                &shown.display().to_string(),
                regex,
                out,
            );
        }
    }
}

pub struct RunCommand;

#[async_trait]
impl Tool for RunCommand {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "run_command".into(),
            description: "Run a shell command in the working directory and return its combined stdout and stderr, plus the exit code when it is non-zero. Use it to build, test, run linters, and inspect the environment. Prefer `read_file`, `edit_file`, `grep`, and `list_dir` for file work. The command is non-interactive: it cannot prompt, and it is killed at the timeout.".into(),
            parameters: schema(
                &json!({
                    "command": {
                        "type": "string",
                        "description": "Shell command, for example `cargo test -p uji`.",
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Seconds before the command is killed. Defaults to 120.",
                        "minimum": 1,
                    },
                }),
                &["command"],
            ),
        }
    }

    fn subject(&self, args: &Value) -> String {
        arg(args, "command")
    }

    async fn run(&self, args: &Value, cwd: &Path) -> Result<String, String> {
        let command = required(args, "command")?;
        let timeout = args
            .get("timeout")
            .and_then(Value::as_u64)
            .unwrap_or(120)
            .max(1);
        let output = tokio::time::timeout(
            Duration::from_secs(timeout),
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&command)
                .current_dir(cwd)
                .output(),
        )
        .await
        .map_err(|_| format!("command timed out after {timeout}s"))?
        .map_err(|err| format!("spawn command: {err}"))?;

        let mut text = String::new();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stdout.is_empty() {
            text.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&stderr);
        }
        if output.status.success() {
            if text.trim().is_empty() {
                text.push_str("(no output, exit code 0)");
            }
        } else {
            let code = output
                .status
                .code()
                .map_or_else(|| String::from("signal"), |code| code.to_string());
            if !text.is_empty() {
                text.push('\n');
            }
            let _ = write!(text, "(exit code {code})");
        }
        Ok(cap(text, MAX_TOOL_OUTPUT))
    }
}
