use std::fmt::Write as _;
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use serde_json::{Value, json};

use crate::llm::ToolSpec;
use crate::process;
use crate::tools::{descriptions, lines};

use super::{Invocation, Tool};

const MAX_READ_LINES: usize = 2_000;
const MAX_READ_BYTES: usize = 50 * 1024;
const MAX_LINE_BYTES: usize = 2_000;
const SNIFF_BYTES: usize = 8_192;
const MAX_TOOL_OUTPUT: usize = 24_000;

fn schema(props: &Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": props,
        "required": required,
        "additionalProperties": false,
    })
}

#[derive(Clone, Default)]
/// Where file tools may reach. Unconfined by default: the whole filesystem is
/// reachable. Turning confinement on restricts tools to the working directory
/// plus any explicitly granted roots.
pub struct Roots {
    extra: Arc<[PathBuf]>,
    confined: bool,
}

impl Roots {
    pub fn new(extra: Vec<PathBuf>) -> Self {
        Self {
            extra: extra.into(),
            confined: false,
        }
    }

    pub fn confined(extra: Vec<PathBuf>) -> Self {
        Self {
            extra: extra.into(),
            confined: true,
        }
    }

    pub fn set_confined(&mut self, confined: bool) {
        self.confined = confined;
    }

    fn candidates<'a>(&'a self, cwd: &'a Path) -> impl Iterator<Item = &'a Path> {
        let anywhere = (!self.confined).then(|| Path::new(ROOT));
        std::iter::once(cwd)
            .chain(self.extra.iter().map(PathBuf::as_path))
            .chain(anywhere)
    }
}

#[cfg(windows)]
const ROOT: &str = "\\";
#[cfg(not(windows))]
const ROOT: &str = "/";

struct Confined {
    dir: Dir,
    rel: PathBuf,
}

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

fn fs_error(action: &str, path: &str, cwd: &Path, err: &std::io::Error) -> String {
    if err.kind() == std::io::ErrorKind::PermissionDenied {
        outside(path, cwd)
    } else {
        format!("{action} {path}: {err}")
    }
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

fn spill_path(command: &str) -> Option<PathBuf> {
    let dir = std::env::temp_dir().join("uji-output");
    std::fs::create_dir_all(&dir).ok()?;
    let stem: String = command
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .take(40)
        .collect();
    Some(dir.join(format!("{}-{stem}.log", std::process::id())))
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
            description: descriptions::read_file::TOOL.into(),
            parameters: schema(
                &json!({
                    "path": {
                        "type": "string",
                        "description": descriptions::read_file::PATH,
                    },
                    "offset": {
                        "type": "integer",
                        "description": descriptions::read_file::OFFSET,
                        "minimum": 1,
                    },
                    "limit": {
                        "type": "integer",
                        "description": descriptions::read_file::LIMIT,
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

    async fn run(&self, args: &Value, call: &Invocation<'_>) -> Result<String, String> {
        let cwd = call.cwd;
        let path = required(args, "path")?;
        let target = resolve(cwd, &path, &self.roots)?;
        if target.dir.metadata(&target.rel).is_ok_and(|m| m.is_dir()) {
            return Err(format!("{path} is a directory, not a file"));
        }
        let file = target
            .dir
            .open(&target.rel)
            .map_err(|err| fs_error("read", &path, cwd, &err))?;
        let mut reader = BufReader::with_capacity(SNIFF_BYTES, file);
        let head = reader
            .fill_buf()
            .map_err(|err| fs_error("read", &path, cwd, &err))?;
        if is_probably_binary(head) {
            return Err(format!("{path} looks like a binary file"));
        }
        let offset = usize_arg(args, "offset").unwrap_or(1).max(1);
        let limit = usize_arg(args, "limit").unwrap_or(MAX_READ_LINES).max(1);

        let mut page = lines::Page::new(offset, limit, MAX_READ_BYTES);
        let mut line = String::new();
        while let lines::Line::Read { truncated } =
            lines::read(&mut reader, &mut line, MAX_LINE_BYTES)
                .map_err(|err| fs_error("read", &path, cwd, &err))?
        {
            page.push(&line, truncated);
        }
        match page.total() {
            0 => Ok(format!("{path} is empty")),
            total if offset > total => Err(format!(
                "offset {offset} is past the end of {path} ({total} lines)"
            )),
            _ => Ok(page.finish()),
        }
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
            description: descriptions::edit_file::TOOL.into(),
            parameters: schema(
                &json!({
                    "path": {
                        "type": "string",
                        "description": descriptions::edit_file::PATH,
                    },
                    "old_string": {
                        "type": "string",
                        "description": descriptions::edit_file::OLD_STRING,
                    },
                    "new_string": {
                        "type": "string",
                        "description": descriptions::edit_file::NEW_STRING,
                    },
                    "replace_all": {
                        "type": "boolean",
                        "description": descriptions::edit_file::REPLACE_ALL,
                    },
                }),
                &["path", "old_string", "new_string"],
            ),
        }
    }

    fn subject(&self, args: &Value) -> String {
        arg(args, "path")
    }

    async fn run(&self, args: &Value, call: &Invocation<'_>) -> Result<String, String> {
        let cwd = call.cwd;
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
            description: descriptions::write_file::TOOL.into(),
            parameters: schema(
                &json!({
                    "path": {
                        "type": "string",
                        "description": descriptions::write_file::PATH,
                    },
                    "content": {
                        "type": "string",
                        "description": descriptions::write_file::CONTENT,
                    },
                }),
                &["path", "content"],
            ),
        }
    }

    fn subject(&self, args: &Value) -> String {
        arg(args, "path")
    }

    async fn run(&self, args: &Value, call: &Invocation<'_>) -> Result<String, String> {
        let cwd = call.cwd;
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

pub struct RunCommand {
    pub roots: Roots,
}

#[async_trait]
impl Tool for RunCommand {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "run_command".into(),
            description: descriptions::run_command::TOOL.into(),
            parameters: schema(
                &json!({
                    "command": {
                        "type": "string",
                        "description": descriptions::run_command::COMMAND,
                    },
                    "timeout": {
                        "type": "integer",
                        "description": descriptions::run_command::TIMEOUT,
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

    async fn run(&self, args: &Value, call: &Invocation<'_>) -> Result<String, String> {
        let command = required(args, "command")?;
        let timeout = args
            .get("timeout")
            .and_then(Value::as_u64)
            .unwrap_or(120)
            .max(1);
        let mut capture = process::Capture::new(MAX_TOOL_OUTPUT);
        if let Some(path) = spill_path(&command)
            && resolve(call.cwd, &path.to_string_lossy(), &self.roots).is_ok()
        {
            capture = capture.spilling(path);
        }
        let spec = process::Spec::shell(&command)
            .in_dir(call.cwd)
            .within(Duration::from_secs(timeout));
        let exit = process::stream(spec, call.cancel, |_, line| {
            capture.push(&line);
            call.progress.send(line);
        })
        .await
        .map_err(|err| format!("spawn command: {err}"))?;

        let mut text = capture.finish();
        match exit {
            process::Exit::TimedOut => return Err(format!("command timed out after {timeout}s")),
            process::Exit::Cancelled => return Err(String::from("command interrupted")),
            process::Exit::Code(0) => {
                if text.trim().is_empty() {
                    text.push_str("(no output, exit code 0)");
                }
            }
            process::Exit::Code(code) => {
                if !text.is_empty() {
                    text.push('\n');
                }
                let _ = write!(text, "(exit code {code})");
            }
        }
        Ok(text)
    }
}

/// Read `lines` of context centred on `line`, for previewing a search hit.
///
/// Goes through the same [`resolve`] confinement as the file tools, so a
/// preview can never reach somewhere a tool could not.
pub fn read_around(
    cwd: &Path,
    path: &str,
    line: usize,
    lines: usize,
    roots: &Roots,
) -> Result<Vec<String>, String> {
    let target = resolve(cwd, path, roots)?;
    let bytes = target
        .dir
        .read(&target.rel)
        .map_err(|err| fs_error("read", path, cwd, &err))?;
    if is_probably_binary(&bytes) {
        return Err(format!("{path} looks like a binary file"));
    }
    let text = String::from_utf8_lossy(&bytes);
    let start = line.saturating_sub(1).saturating_sub(lines / 4);
    Ok(text
        .lines()
        .enumerate()
        .skip(start)
        .take(lines)
        .map(|(at, text)| format!("{:>5}| {text}", at.saturating_add(1)))
        .collect())
}
