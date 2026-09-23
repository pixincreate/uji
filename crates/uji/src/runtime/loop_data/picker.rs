use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::LoopData;
use crate::api::modal;

const CONTEXT_LINES: usize = 40;

/// How long the query must sit still before a live picker re-queries, so a
/// burst of keystrokes spawns one job rather than one per character.
const DEBOUNCE: Duration = Duration::from_millis(120);

/// A live picker's query: what is waiting out the debounce, and what has
/// already been dispatched.
///
/// Both halves are needed. Without `pending` every keystroke spawns a job;
/// without `sent` a settled query is dispatched again on every tick, and each
/// answer resets the cursor under the user.
#[derive(Default)]
pub(crate) struct LiveQuery {
    pending: Option<(String, Instant)>,
    sent: Option<String>,
}

impl LiveQuery {
    /// The query to dispatch now, if typing has settled on something new.
    fn due(&mut self, query: &str) -> Option<String> {
        if self.sent.as_deref() == Some(query) {
            self.pending = None;
            return None;
        }
        match &self.pending {
            Some((pending, since)) if pending == query => {
                if since.elapsed() < DEBOUNCE {
                    return None;
                }
                self.pending = None;
                self.sent = Some(query.to_string());
                Some(query.to_string())
            }
            _ => {
                self.pending = Some((query.to_string(), Instant::now()));
                None
            }
        }
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

impl LoopData {
    /// Keep the open picker current: dispatch a settled query, and fetch the
    /// preview for whatever is highlighted.
    pub(super) fn sync_picker(&mut self) {
        self.dispatch_query();
        self.load_preview();
    }

    /// Ask a live picker's source for fresh candidates once typing settles.
    fn dispatch_query(&mut self) {
        let Some(query) = self.app.live_query().map(str::to_string) else {
            self.live_query.clear();
            return;
        };
        let Some(query) = self.live_query.due(&query) else {
            return;
        };
        let Some(hook) = self.inner.api.pick().borrow().query() else {
            return;
        };
        let token = self.inner.api.pick().borrow_mut().next_token();
        let called = modal::show(&self.inner.lua, &self.inner.api, token)
            .and_then(|show| hook.call::<()>((query, show)));
        if let Err(err) = called {
            self.inner.notify(format!("pick query: {err}"));
        }
    }

    /// Fill the preview pane for whatever is highlighted.
    ///
    /// A picker may supply its own `preview` function; otherwise items that look
    /// like `path:line:` — what `rg --line-number` emits — are previewed from
    /// the file, through the same confinement tools use.
    fn load_preview(&mut self) {
        if !self.app.preview_pending() {
            return;
        }
        let Some((_, item)) = self.app.picked() else {
            return;
        };
        let item = item.to_string();
        let hook = self.inner.api.pick().borrow().preview();
        let lines = match hook {
            Some(hook) => match hook.call::<Vec<String>>(item) {
                Ok(lines) => lines,
                Err(err) => {
                    self.inner.notify(format!("preview: {err}"));
                    Vec::new()
                }
            },
            None => self.file_preview(&item),
        };
        self.app.set_preview(lines);
        self.dirty = true;
    }

    fn file_preview(&self, item: &str) -> Vec<String> {
        let Some((path, line)) = split_location(item) else {
            return Vec::new();
        };
        let cwd = PathBuf::from(&self.app.messages().info().directory);
        let files = self.inner.api.access().borrow().files(cwd);
        files
            .around(path, line, CONTEXT_LINES)
            .unwrap_or_else(|err| vec![err.to_string()])
    }
}

/// `path:line:rest` as emitted by `rg --line-number --no-heading`.
fn split_location(item: &str) -> Option<(&str, usize)> {
    let (path, rest) = item.split_once(':')?;
    let line = rest
        .split_once(':')
        .map_or(rest, |(number, _)| number)
        .trim()
        .parse::<usize>()
        .ok()?;
    (!path.is_empty()).then_some((path, line))
}
