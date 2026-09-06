use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use notify::{RecursiveMode, Watcher};

const RELOAD_DEBOUNCE: u64 = 150;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ConfigEvent {
    Reload,
}

pub(crate) fn watch(
    paths: Vec<PathBuf>,
    sender: calloop::channel::Sender<ConfigEvent>,
) -> notify::Result<notify::RecommendedWatcher> {
    let last_send = Arc::new(AtomicU64::new(0));
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        if result.is_err() {
            return;
        }
        let now = u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
        )
        .unwrap_or(u64::MAX);
        let previous = last_send.load(Ordering::Relaxed);
        if now.saturating_sub(previous) >= RELOAD_DEBOUNCE
            && last_send
                .compare_exchange(previous, now, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
        {
            let _ = sender.send(ConfigEvent::Reload);
        }
    })?;
    for path in paths {
        watcher.watch(&path, RecursiveMode::NonRecursive)?;
    }
    Ok(watcher)
}
