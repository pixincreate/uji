use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use mlua::{BString, Lua};
use serde::Serialize;
use tokio::io::AsyncWriteExt;
use uji_macros::{FromLua, IntoLua, function};

use crate::io;

#[derive(Serialize, IntoLua)]
struct Entry {
    name: String,
    #[serde(rename = "type")]
    kind: &'static str,
}

#[derive(Serialize, IntoLua)]
struct Stat {
    #[serde(rename = "type")]
    kind: &'static str,
    size: u64,
    modified: Option<i64>,
}

fn kind(file_type: std::fs::FileType) -> &'static str {
    if file_type.is_dir() {
        "dir"
    } else if file_type.is_file() {
        "file"
    } else if file_type.is_symlink() {
        "link"
    } else {
        "other"
    }
}

fn stat_of(metadata: &std::fs::Metadata) -> Stat {
    Stat {
        kind: kind(metadata.file_type()),
        size: metadata.len(),
        modified: metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok()),
    }
}

const NEWLINE: u8 = b'\n';
const BUFFER: usize = 64 * 1024;

#[derive(Serialize, Default, IntoLua)]
struct Excerpt {
    lines: Vec<String>,
    cut: Vec<usize>,
    total: Option<usize>,
    binary: bool,
}

#[derive(FromLua)]
struct Window {
    #[lua(default = 1)]
    from: usize,
    #[lua(default = usize::MAX)]
    count: usize,
    #[lua(default = usize::MAX)]
    max: usize,
    #[lua(default)]
    sniff: usize,
    #[lua(default)]
    total: bool,
}

#[derive(FromLua)]
struct WriteOptions {
    mode: Option<u32>,
    #[lua(default)]
    append: bool,
}

#[derive(FromLua)]
struct RemoveOptions {
    #[lua(default)]
    recursive: bool,
}

impl Window {
    fn read(&self, path: &str) -> std::io::Result<Excerpt> {
        let file = std::fs::File::open(path)?;
        let mut reader = BufReader::with_capacity(BUFFER.max(self.sniff), file);
        let mut excerpt = Excerpt::default();
        if self.sniff > 0 {
            let head = reader.fill_buf()?;
            if head[..head.len().min(self.sniff)].contains(&0) {
                excerpt.binary = true;
                return Ok(excerpt);
            }
        }
        let mut number = 0usize;
        let mut line = Vec::new();
        loop {
            let wanted = number.saturating_add(1) >= self.from && excerpt.lines.len() < self.count;
            if !wanted && !self.total && excerpt.lines.len() >= self.count {
                break;
            }
            let keep = if wanted { self.max } else { 0 };
            let Some(truncated) = next_line(&mut reader, &mut line, keep)? else {
                break;
            };
            number = number.saturating_add(1);
            if wanted {
                excerpt
                    .lines
                    .push(String::from_utf8_lossy(&line).into_owned());
                if truncated {
                    excerpt.cut.push(excerpt.lines.len());
                }
            }
        }
        if self.total {
            excerpt.total = Some(number);
        }
        Ok(excerpt)
    }
}

fn next_line(
    reader: &mut impl BufRead,
    line: &mut Vec<u8>,
    keep: usize,
) -> std::io::Result<Option<bool>> {
    line.clear();
    let mut started = false;
    let mut truncated = false;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(started.then_some(truncated));
        }
        started = true;
        let ended = available.iter().position(|byte| *byte == NEWLINE);
        let chunk = ended.map_or(available, |at| &available[..at]);
        let take = chunk.len().min(keep.saturating_sub(line.len()));
        line.extend_from_slice(&chunk[..take]);
        truncated |= take < chunk.len();
        let consumed = chunk.len().saturating_add(usize::from(ended.is_some()));
        reader.consume(consumed);
        if ended.is_some() {
            return Ok(Some(truncated));
        }
    }
}

struct Write {
    path: PathBuf,
    data: Vec<u8>,
    mode: Option<u32>,
    append: bool,
}

impl Write {
    fn new(path: String, data: &mlua::LuaString, opts: &WriteOptions) -> Self {
        Self {
            path: PathBuf::from(path),
            data: data.as_bytes().to_vec(),
            mode: opts.mode,
            append: opts.append,
        }
    }

    async fn run(self) -> std::io::Result<()> {
        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create(true);
        if self.append {
            options.append(true);
        } else {
            options.truncate(true);
        }
        #[cfg(unix)]
        if let Some(mode) = self.mode {
            options.mode(mode);
        }
        let mut file = options.open(&self.path).await?;
        file.write_all(&self.data).await?;
        file.flush().await
    }
}

async fn entries(path: String) -> std::io::Result<Vec<Entry>> {
    let mut reader = tokio::fs::read_dir(path).await?;
    let mut entries = Vec::new();
    while let Some(entry) = reader.next_entry().await? {
        entries.push(Entry {
            name: entry.file_name().to_string_lossy().into_owned(),
            kind: kind(entry.file_type().await?),
        });
    }
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(entries)
}

#[function(fs)]
async fn read(lua: Lua, path: String) -> mlua::Result<Result<BString, std::io::Error>> {
    Ok(io::run(&lua, tokio::fs::read(path))
        .await?
        .map(BString::from))
}

#[function(fs)]
async fn lines(
    lua: Lua,
    path: String,
    window: Window,
) -> mlua::Result<Result<Excerpt, std::io::Error>> {
    io::blocking(&lua, move || window.read(&path)).await
}

#[function(fs)]
async fn write(
    lua: Lua,
    path: String,
    data: &mlua::LuaString,
    opts: &WriteOptions,
) -> mlua::Result<Result<bool, std::io::Error>> {
    let write = Write::new(path, data, opts);
    Ok(io::run(&lua, write.run()).await?.map(|()| true))
}

#[function(fs)]
async fn list(lua: Lua, path: String) -> mlua::Result<Result<Vec<Entry>, std::io::Error>> {
    io::run(&lua, entries(path)).await
}

#[function(fs)]
async fn stat(lua: Lua, path: String) -> mlua::Result<Result<Stat, std::io::Error>> {
    let metadata = io::run(&lua, tokio::fs::metadata(path)).await?;
    Ok(metadata.map(|metadata| stat_of(&metadata)))
}

#[function(fs)]
async fn mkdir(lua: Lua, path: String) -> mlua::Result<Result<bool, std::io::Error>> {
    Ok(io::run(&lua, tokio::fs::create_dir_all(path))
        .await?
        .map(|()| true))
}

#[function(fs)]
async fn remove(
    lua: Lua,
    path: String,
    opts: RemoveOptions,
) -> mlua::Result<Result<bool, std::io::Error>> {
    Ok(io::run(&lua, delete(path, opts.recursive))
        .await?
        .map(|()| true))
}

#[function(fs)]
async fn rename(lua: Lua, from: String, to: String) -> mlua::Result<Result<bool, std::io::Error>> {
    Ok(io::run(&lua, tokio::fs::rename(from, to))
        .await?
        .map(|()| true))
}

#[function(fs)]
async fn realpath(lua: Lua, path: String) -> mlua::Result<Result<String, std::io::Error>> {
    let resolved = io::run(&lua, tokio::fs::canonicalize(path)).await?;
    Ok(resolved.map(|path| path.display().to_string()))
}

async fn delete(path: String, recursive: bool) -> std::io::Result<()> {
    let metadata = tokio::fs::symlink_metadata(&path).await?;
    if !metadata.is_dir() {
        return tokio::fs::remove_file(path).await;
    }
    if recursive {
        tokio::fs::remove_dir_all(path).await
    } else {
        tokio::fs::remove_dir(path).await
    }
}
