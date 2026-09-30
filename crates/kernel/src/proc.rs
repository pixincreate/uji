use std::collections::HashMap;
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::runtime::Handle;
use tokio::sync::{Mutex, mpsc, watch};
use uji_native::{Held, Json, List, native};

use crate::context;

const NEWLINE: u8 = b'\n';
const RETURN: u8 = b'\r';

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum Stream {
    Stdout,
    Stderr,
}

struct Line {
    stream: Stream,
    text: String,
}

#[derive(Clone, Copy, Serialize)]
struct Exit {
    code: Option<i32>,
    signal: Option<i32>,
    success: bool,
}

impl Exit {
    const LOST: Self = Self {
        code: None,
        signal: None,
        success: false,
    };

    fn of(status: &std::io::Result<ExitStatus>) -> Self {
        let Ok(status) = status else {
            return Self::LOST;
        };
        Self {
            code: status.code(),
            signal: signal(*status),
            success: status.success(),
        }
    }
}

#[cfg(unix)]
fn signal(status: ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn signal(_: ExitStatus) -> Option<i32> {
    None
}

pub(crate) struct Proc {
    stdin: Mutex<Option<ChildStdin>>,
    output: Mutex<mpsc::UnboundedReceiver<Line>>,
    status: watch::Receiver<Option<Exit>>,
    kill: mpsc::UnboundedSender<()>,
}

impl Proc {
    fn start(io: &Handle, mut child: Child) -> Self {
        let (lines, output) = mpsc::unbounded_channel();
        if let Some(stdout) = child.stdout.take() {
            io.spawn(pipe(stdout, Stream::Stdout, lines.clone()));
        }
        if let Some(stderr) = child.stderr.take() {
            io.spawn(pipe(stderr, Stream::Stderr, lines));
        }
        let (report, status) = watch::channel(None);
        let (kill, killed) = mpsc::unbounded_channel();
        let stdin = child.stdin.take();
        io.spawn(supervise(child, killed, report));
        Self {
            stdin: Mutex::new(stdin),
            output: Mutex::new(output),
            status,
            kill,
        }
    }
}

async fn pipe(reader: impl AsyncRead + Unpin, stream: Stream, lines: mpsc::UnboundedSender<Line>) {
    let mut reader = BufReader::new(reader);
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        match reader.read_until(NEWLINE, &mut buffer).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {
                if buffer.last() == Some(&NEWLINE) {
                    buffer.pop();
                }
                if buffer.last() == Some(&RETURN) {
                    buffer.pop();
                }
                let text = String::from_utf8_lossy(&buffer).into_owned();
                if lines.send(Line { stream, text }).is_err() {
                    return;
                }
            }
        }
    }
}

async fn supervise(
    mut child: Child,
    mut killed: mpsc::UnboundedReceiver<()>,
    report: watch::Sender<Option<Exit>>,
) {
    let status = tokio::select! {
        status = child.wait() => status,
        _ = killed.recv() => {
            drop(child.start_kill());
            child.wait().await
        }
    };
    report.send_replace(Some(Exit::of(&status)));
}

#[native(iterate = lines)]
async fn line(process: Arc<Proc>) -> Option<(String, Stream)> {
    let line = process.output.lock().await.recv().await?;
    Some((line.text, line.stream))
}

#[native]
async fn write(process: Arc<Proc>, data: Vec<u8>) -> std::io::Result<()> {
    let mut stdin = process.stdin.lock().await;
    let pipe = stdin
        .as_mut()
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::BrokenPipe))?;
    pipe.write_all(&data).await?;
    pipe.flush().await
}

#[native]
async fn close(process: Arc<Proc>) {
    process.stdin.lock().await.take();
}

#[native]
fn kill(process: &Arc<Proc>) -> bool {
    process.kill.send(()).is_ok()
}

#[native]
async fn wait(process: Arc<Proc>) -> Json<Exit> {
    let mut status = process.status.clone();
    let exit = status
        .wait_for(Option::is_some)
        .await
        .ok()
        .and_then(|exit| *exit);
    Json(exit.unwrap_or(Exit::LOST))
}

#[derive(Deserialize)]
struct SpawnOptions {
    cwd: Option<String>,
    env: Option<HashMap<String, String>>,
    stdio: Option<String>,
}

#[derive(Serialize)]
struct Spawned {
    pid: Option<u32>,
}

fn command(line: &[String], opts: SpawnOptions) -> std::io::Result<Command> {
    let Some((program, args)) = line.split_first() else {
        return Err(std::io::Error::other("proc.spawn needs a program"));
    };
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    match opts.stdio.as_deref() {
        None | Some("pipe") => {}
        Some("inherit") => {
            command
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
        }
        Some(other) => {
            return Err(std::io::Error::other(format!(
                "stdio must be \"pipe\" or \"inherit\", not {other:?}"
            )));
        }
    }
    if let Some(cwd) = opts.cwd {
        command.current_dir(cwd);
    }
    if let Some(env) = opts.env {
        command.envs(env);
    }
    Ok(command)
}

#[native(proc)]
fn spawn(
    argv: Json<List<String>>,
    opts: Json<SpawnOptions>,
) -> std::io::Result<Held<Arc<Proc>, Spawned>> {
    let Json(List(argv)) = argv;
    let mut command = command(&argv, opts.0)?;
    let io = context::with(|context| context.io.clone())
        .ok_or_else(|| std::io::Error::other("the kernel is not running"))?;
    let entered = io.enter();
    let spawned = command.spawn();
    drop(entered);
    let child = spawned?;
    let pid = child.id();
    Ok(Held(Arc::new(Proc::start(&io, child)), Spawned { pid }))
}
