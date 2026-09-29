use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};
use uji_native::{Json, List, native};

struct Candidate {
    at: usize,
    text: String,
}

impl AsRef<str> for Candidate {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

#[native]
fn fuzzy(query: &str, items: Json<List<String>>) -> Json<Vec<usize>> {
    let items = items.0.0;
    if query.is_empty() {
        return Json((1..=items.len()).collect());
    }
    let candidates = items.into_iter().enumerate().map(|(at, text)| Candidate {
        at: at.saturating_add(1),
        text,
    });
    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    Json(
        Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart)
            .match_list(candidates, &mut matcher)
            .into_iter()
            .map(|(candidate, _)| candidate.at)
            .collect(),
    )
}
