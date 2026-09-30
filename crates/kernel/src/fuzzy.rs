use mlua::LuaString;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};
use uji_macros::function;

struct Candidate<'a> {
    at: usize,
    text: &'a str,
}

impl AsRef<str> for Candidate<'_> {
    fn as_ref(&self) -> &str {
        self.text
    }
}

#[function]
fn fuzzy(query: &str, items: &[LuaString]) -> Vec<usize> {
    if query.is_empty() {
        return (1..=items.len()).collect();
    }
    let texts: Vec<String> = items.iter().map(LuaString::to_string_lossy).collect();
    let candidates = texts.iter().enumerate().map(|(at, text)| Candidate {
        at: at.saturating_add(1),
        text,
    });
    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart)
        .match_list(candidates, &mut matcher)
        .into_iter()
        .map(|(candidate, _)| candidate.at)
        .collect()
}
