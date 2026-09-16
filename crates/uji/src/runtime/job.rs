use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};

use uji_agent::llm::CancelToken;

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

#[derive(Default)]
pub(crate) struct Running {
    tokens: HashMap<u64, CancelToken>,
}

impl Running {
    pub(crate) fn insert(&mut self, id: u64, token: CancelToken) {
        self.tokens.insert(id, token);
    }

    pub(crate) fn stop(&mut self, id: u64) {
        if let Some(token) = self.tokens.remove(&id) {
            token.cancel();
        }
    }

    pub(crate) fn finish(&mut self, id: u64) {
        self.tokens.remove(&id);
    }
}

async fn next_line<R>(reader: &mut Option<tokio::io::Lines<BufReader<R>>>) -> Option<String>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let lines = reader.as_mut()?;
    lines.next_line().await.ok().flatten()
}

pub(crate) async fn run(
    id: u64,
    command: Vec<String>,
    cwd: Option<PathBuf>,
    cancel: CancelToken,
    send: impl Fn(JobEvent) + Send + 'static,
) {
    let Some((program, args)) = command.split_first() else {
        send(JobEvent::Exit { id, code: -1 });
        return;
    };
    let mut builder = tokio::process::Command::new(program);
    builder
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        builder.current_dir(cwd);
    }
    let mut child = match builder.spawn() {
        Ok(child) => child,
        Err(err) => {
            send(JobEvent::Stderr {
                id,
                line: format!("spawn {program}: {err}"),
            });
            send(JobEvent::Exit { id, code: -1 });
            return;
        }
    };

    let mut stdout = child.stdout.take().map(|pipe| BufReader::new(pipe).lines());
    let mut stderr = child.stderr.take().map(|pipe| BufReader::new(pipe).lines());

    loop {
        tokio::select! {
            line = next_line(&mut stdout), if stdout.is_some() => {
                match line {
                    Some(line) => send(JobEvent::Stdout { id, line }),
                    None => stdout = None,
                }
            }
            line = next_line(&mut stderr), if stderr.is_some() => {
                match line {
                    Some(line) => send(JobEvent::Stderr { id, line }),
                    None => stderr = None,
                }
            }
            status = child.wait(), if stdout.is_none() && stderr.is_none() => {
                let code = status.ok().and_then(|s| s.code()).unwrap_or(-1);
                send(JobEvent::Exit { id, code });
                return;
            }
            () = cancel.cancelled() => {
                let _ = child.kill().await;
                send(JobEvent::Exit { id, code: -1 });
                return;
            }
        }
    }
}
