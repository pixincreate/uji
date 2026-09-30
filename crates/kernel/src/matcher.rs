use globset::{GlobBuilder, GlobMatcher};
use mlua::LuaString;
use regex::bytes::Regex;
use uji_macros::{function, methods, options};

pub(crate) enum Matcher {
    Regex(Regex),
    Glob(GlobMatcher),
}

impl Matcher {
    fn search(&self, subject: &[u8]) -> Option<(usize, usize)> {
        match self {
            Self::Regex(regex) => regex
                .find(subject)
                .map(|found| (found.start(), found.end())),
            Self::Glob(glob) => glob
                .is_match(&*String::from_utf8_lossy(subject))
                .then_some((0, subject.len())),
        }
    }
}

#[methods]
impl Matcher {
    fn test(&self, subject: &LuaString) -> bool {
        self.search(&subject.as_bytes()).is_some()
    }

    fn find(&self, subject: &LuaString) -> (Option<usize>, Option<usize>) {
        self.search(&subject.as_bytes())
            .map(|(start, end)| (start.saturating_add(1), end))
            .unzip()
    }
}

#[options]
struct GlobOptions {
    #[serde(default)]
    separator: bool,
}

#[function]
fn regex(pattern: &str) -> Result<Matcher, regex::Error> {
    Regex::new(pattern).map(Matcher::Regex)
}

#[function]
fn glob(pattern: &str, opts: &GlobOptions) -> Result<Matcher, globset::Error> {
    GlobBuilder::new(pattern)
        .literal_separator(opts.separator)
        .build()
        .map(|glob| Matcher::Glob(glob.compile_matcher()))
}
