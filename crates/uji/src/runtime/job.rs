use std::path::PathBuf;

use tokio::sync::mpsc::UnboundedReceiver;
use uji_agent::llm::CancelToken;
use uji_agent::process::{self, Exit, Spec, Stream};

pub(crate) enum JobEvent {
    Stdout { id: u64, line: String },
    Stderr { id: u64, line: String },
    Exit { id: u64, code: i32 },
}

impl JobEvent {
    pub(crate) fn id(&self) -> u64 {
        match self {
            Self::Stdout { id, .. } | Self::Stderr { id, .. } | Self::Exit { id, .. } => *id,
        }
    }
}

pub(crate) type Write = Option<String>;

pub(crate) async fn run(
    id: u64,
    command: Vec<String>,
    cwd: Option<PathBuf>,
    cancel: CancelToken,
    writes: UnboundedReceiver<Write>,
    send: impl Fn(JobEvent) + Send + 'static,
) {
    let mut spec = Spec::argv(&command).writing(writes);
    if let Some(cwd) = cwd.as_deref() {
        spec = spec.in_dir(cwd);
    }
    let exit = process::stream(spec, &cancel, |stream, line| {
        send(match stream {
            Stream::Out => JobEvent::Stdout { id, line },
            Stream::Err => JobEvent::Stderr { id, line },
        });
    })
    .await;
    let code = match exit {
        Ok(Exit::Code(code)) => code,
        Ok(_) => -1,
        Err(err) => {
            send(JobEvent::Stderr {
                id,
                line: format!("spawn: {err}"),
            });
            -1
        }
    };
    send(JobEvent::Exit { id, code });
}
