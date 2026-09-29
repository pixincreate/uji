use globset::{GlobBuilder, GlobMatcher};
use regex::Regex;
use serde::Deserialize;
use uji_native::{Held, Json, native};

pub(crate) enum Matcher {
    Regex(Regex),
    Glob(GlobMatcher),
}

impl Matcher {
    fn search(&self, subject: &str) -> Option<(usize, usize)> {
        match self {
            Self::Regex(regex) => regex
                .find(subject)
                .map(|found| (found.start(), found.end())),
            Self::Glob(glob) => glob.is_match(subject).then_some((0, subject.len())),
        }
    }
}

#[derive(Deserialize)]
struct GlobOptions {
    #[serde(default)]
    separator: bool,
}

#[native]
fn regex(pattern: &str) -> Result<Held<Matcher>, regex::Error> {
    Regex::new(pattern).map(|regex| Held::new(Matcher::Regex(regex)))
}

#[native]
fn glob(pattern: &str, opts: Json<GlobOptions>) -> Result<Held<Matcher>, globset::Error> {
    let Json(opts) = opts;
    GlobBuilder::new(pattern)
        .literal_separator(opts.separator)
        .build()
        .map(|glob| Held::new(Matcher::Glob(glob.compile_matcher())))
}

#[native]
fn test(matcher: &Matcher, subject: &str) -> bool {
    matcher.search(subject).is_some()
}

#[native]
fn find(matcher: &Matcher, subject: &str) -> Option<(usize, usize)> {
    matcher
        .search(subject)
        .map(|(start, end)| (start.saturating_add(1), end))
}
