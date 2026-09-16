use std::path::PathBuf;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc::UnboundedReceiver;

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

async fn next_line<R>(reader: &mut Option<tokio::io::Lines<BufReader<R>>>) -> Option<String>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let lines = reader.as_mut()?;
    lines.next_line().await.ok().flatten()
}

pub(crate) type Write = Option<String>;

pub(crate) async fn run(
    id: u64,
    command: Vec<String>,
    cwd: Option<PathBuf>,
    cancel: CancelToken,
    mut writes: UnboundedReceiver<Write>,
    send: impl Fn(JobEvent) + Send + 'static,
) {
    let Some((program, args)) = command.split_first() else {
        send(JobEvent::Exit { id, code: -1 });
        return;
    };
    let mut builder = tokio::process::Command::new(program);
    builder
        .args(args)
        .stdin(Stdio::piped())
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
    let mut stdin = child.stdin.take();

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
            write = writes.recv(), if stdin.is_some() => {
                match write {
                    Some(Some(data)) => {
                        if let Some(pipe) = stdin.as_mut()
                            && pipe.write_all(data.as_bytes()).await.is_err() {
                            stdin = None;
                        }
                    }
                    _ => stdin = None,
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
