use tokio::sync::mpsc::UnboundedSender;

/// A tool's live output channel. Tools that take a while report what they are
/// doing through this instead of staying silent until they finish.
#[derive(Clone, Default)]
pub struct Progress {
    sink: Option<UnboundedSender<String>>,
}

impl Progress {
    pub fn new(sink: UnboundedSender<String>) -> Self {
        Self { sink: Some(sink) }
    }

    /// Report output produced so far. Dropped if nothing is listening.
    pub fn send(&self, chunk: impl Into<String>) {
        if let Some(sink) = &self.sink {
            let _ = sink.send(chunk.into());
        }
    }
}
