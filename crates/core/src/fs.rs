mod lines;

use std::io::{BufRead, BufReader, ErrorKind};
use std::path::{Component, Path, PathBuf};

use cap_std::ambient_authority;
use cap_std::fs::Dir;

use self::lines::Line;

const SNIFF_BYTES: usize = 8_192;
const PREVIEW_LINE: usize = 2_000;

#[cfg(windows)]
const ROOT: &str = "\\";
#[cfg(not(windows))]
const ROOT: &str = "/";

#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error(
        "{path} is outside the working directory ({}); tools can only reach files under it",
        cwd.display()
    )]
    Outside { path: String, cwd: PathBuf },
    #[error("{action} {path}: {source}")]
    Io {
        action: &'static str,
        path: String,
        source: std::io::Error,
    },
    #[error("{0} is a directory, not a file")]
    Directory(String),
    #[error("{0} looks like a binary file")]
    Binary(String),
}

#[derive(Clone, Copy)]
pub struct Window {
    pub offset: usize,
    pub limit: usize,
    pub max_line: usize,
}

#[derive(Default)]
pub struct Excerpt {
    pub lines: Vec<String>,
    pub cut: Vec<usize>,
    pub total: usize,
}

pub struct Written {
    pub created: bool,
}

pub struct Files {
    cwd: PathBuf,
    extra: Vec<PathBuf>,
    confined: bool,
}

struct Target {
    dir: Dir,
    rel: PathBuf,
}

impl Files {
    pub fn new(cwd: PathBuf, extra: Vec<PathBuf>, confined: bool) -> Self {
        Self {
            cwd,
            extra,
            confined,
        }
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>, FsError> {
        let target = self.resolve(path)?;
        target
            .dir
            .read(&target.rel)
            .map_err(|err| self.failed("read", path, err))
    }

    pub fn lines(&self, path: &str, window: Window) -> Result<Excerpt, FsError> {
        let mut reader = self.open_text(path)?;
        let mut out = Excerpt::default();
        let mut line = String::new();
        while let Line::Read { truncated } = lines::read(&mut reader, &mut line, window.max_line)
            .map_err(|err| self.failed("read", path, err))?
        {
            out.total = out.total.saturating_add(1);
            if out.total >= window.offset && out.lines.len() < window.limit {
                out.lines.push(std::mem::take(&mut line));
                if truncated {
                    out.cut.push(out.lines.len());
                }
            }
        }
        Ok(out)
    }

    pub fn write(&self, path: &str, content: &[u8]) -> Result<Written, FsError> {
        let target = self.resolve(path)?;
        let created = target.dir.metadata(&target.rel).is_err();
        if let Some(parent) = target
            .rel
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            target
                .dir
                .create_dir_all(parent)
                .map_err(|err| self.failed("mkdir", path, err))?;
        }
        target
            .dir
            .write(&target.rel, content)
            .map_err(|err| self.failed("write", path, err))?;
        Ok(Written { created })
    }

    pub fn around(&self, path: &str, line: usize, count: usize) -> Result<Vec<String>, FsError> {
        let mut reader = self.open_text(path)?;
        let start = line.saturating_sub(1).saturating_sub(count / 4);
        let end = start.saturating_add(count);
        let mut out = Vec::new();
        let mut text = String::new();
        let mut at = 0usize;
        while at < end
            && lines::read(&mut reader, &mut text, PREVIEW_LINE)
                .map_err(|err| self.failed("read", path, err))?
                != Line::Eof
        {
            at = at.saturating_add(1);
            if at > start {
                let shown = text.strip_suffix('\r').unwrap_or(&text);
                out.push(format!("{at:>5}| {shown}"));
            }
        }
        Ok(out)
    }

    fn open_text(&self, path: &str) -> Result<BufReader<cap_std::fs::File>, FsError> {
        let target = self.resolve(path)?;
        if target
            .dir
            .metadata(&target.rel)
            .is_ok_and(|meta| meta.is_dir())
        {
            return Err(FsError::Directory(path.to_string()));
        }
        let file = target
            .dir
            .open(&target.rel)
            .map_err(|err| self.failed("read", path, err))?;
        let mut reader = BufReader::with_capacity(SNIFF_BYTES, file);
        let head = reader
            .fill_buf()
            .map_err(|err| self.failed("read", path, err))?;
        if head.contains(&0) {
            return Err(FsError::Binary(path.to_string()));
        }
        Ok(reader)
    }

    fn resolve(&self, path: &str) -> Result<Target, FsError> {
        let raw = Path::new(path);
        let anywhere = (!self.confined).then(|| Path::new(ROOT));
        let (root, rel) = std::iter::once(self.cwd.as_path())
            .chain(self.extra.iter().map(PathBuf::as_path))
            .chain(anywhere)
            .find_map(|root| relative_to(root, raw).map(|rel| (root, rel)))
            .ok_or_else(|| self.outside(path))?;
        let dir =
            Dir::open_ambient_dir(root, ambient_authority()).map_err(|source| FsError::Io {
                action: "open",
                path: root.display().to_string(),
                source,
            })?;
        Ok(Target { dir, rel })
    }

    fn outside(&self, path: &str) -> FsError {
        FsError::Outside {
            path: path.to_string(),
            cwd: self.cwd.clone(),
        }
    }

    fn failed(&self, action: &'static str, path: &str, source: std::io::Error) -> FsError {
        if source.kind() == ErrorKind::PermissionDenied {
            self.outside(path)
        } else {
            FsError::Io {
                action,
                path: path.to_string(),
                source,
            }
        }
    }
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
