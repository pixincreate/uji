use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use uji_native::{Json, native};

use crate::io::{self, Blocked};

#[derive(Serialize)]
struct Entry {
    name: String,
    #[serde(rename = "type")]
    kind: &'static str,
}

#[derive(Serialize)]
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

#[derive(Serialize, Default)]
struct Excerpt {
    lines: Vec<String>,
    cut: Vec<usize>,
    total: Option<usize>,
    binary: bool,
}

#[derive(Deserialize)]
struct Window {
    #[serde(default = "first")]
    from: usize,
    #[serde(default = "unlimited")]
    count: usize,
    #[serde(default = "unlimited")]
    max: usize,
    #[serde(default)]
    sniff: usize,
    #[serde(default)]
    total: bool,
}

fn first() -> usize {
    1
}

fn unlimited() -> usize {
    usize::MAX
}

#[derive(Deserialize)]
struct WriteOptions {
    mode: Option<u32>,
    #[serde(default)]
    append: bool,
}

#[derive(Deserialize)]
struct RemoveOptions {
    #[serde(default)]
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

#[native(fs)]
async fn read(path: String) -> std::io::Result<Vec<u8>> {
    tokio::fs::read(path).await
}

#[native(fs)]
async fn lines(
    path: String,
    window: Json<Window>,
) -> Result<Json<Excerpt>, Blocked<std::io::Error>> {
    io::blocking(move || window.0.read(&path)).await.map(Json)
}

#[native(fs)]
async fn write(path: String, data: Vec<u8>, opts: Json<WriteOptions>) -> std::io::Result<()> {
    let Json(opts) = opts;
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create(true);
    if opts.append {
        options.append(true);
    } else {
        options.truncate(true);
    }
    #[cfg(unix)]
    if let Some(mode) = opts.mode {
        options.mode(mode);
    }
    let mut file = options.open(PathBuf::from(path)).await?;
    file.write_all(&data).await?;
    file.flush().await
}

#[native(fs)]
async fn list(path: String) -> std::io::Result<Json<Vec<Entry>>> {
    let mut reader = tokio::fs::read_dir(path).await?;
    let mut entries = Vec::new();
    while let Some(entry) = reader.next_entry().await? {
        entries.push(Entry {
            name: entry.file_name().to_string_lossy().into_owned(),
            kind: kind(entry.file_type().await?),
        });
    }
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(Json(entries))
}

#[native(fs)]
async fn stat(path: String) -> std::io::Result<Json<Stat>> {
    Ok(Json(stat_of(&tokio::fs::metadata(path).await?)))
}

#[native(fs)]
async fn mkdir(path: String) -> std::io::Result<()> {
    tokio::fs::create_dir_all(path).await
}

#[native(fs)]
async fn remove(path: String, opts: Json<RemoveOptions>) -> std::io::Result<()> {
    delete(path, opts.0.recursive).await
}

#[native(fs)]
async fn rename(from: String, to: String) -> std::io::Result<()> {
    tokio::fs::rename(from, to).await
}

#[native(fs)]
async fn realpath(path: String) -> std::io::Result<String> {
    Ok(tokio::fs::canonicalize(path).await?.display().to_string())
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
