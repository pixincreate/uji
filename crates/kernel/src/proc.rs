use std::collections::HashMap;
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;

use mlua::Lua;
use serde::Serialize;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::runtime::Handle;
use tokio::sync::{Mutex, mpsc, watch};
use uji_macros::{FromLua, IntoLua, function, methods};

use crate::io;

const NEWLINE: u8 = b'\n';
const RETURN: u8 = b'\r';

struct Line {
    stream: &'static str,
    text: String,
}

#[derive(Clone, Copy, Serialize, IntoLua)]
struct Exit {
    code: Option<i32>,
    signal: Option<i32>,
    success: bool,
}

impl Exit {
    fn of(status: &std::io::Result<ExitStatus>) -> Self {
        let Ok(status) = status else {
            return Self {
                code: None,
                signal: None,
                success: false,
            };
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
    pid: Option<u32>,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    output: Mutex<mpsc::UnboundedReceiver<Line>>,
    status: watch::Receiver<Option<Exit>>,
    kill: mpsc::UnboundedSender<()>,
}

impl Proc {
    fn start(io: &Handle, mut child: Child) -> Self {
        let (lines, output) = mpsc::unbounded_channel();
        if let Some(stdout) = child.stdout.take() {
            io.spawn(pipe(stdout, "stdout", lines.clone()));
        }
        if let Some(stderr) = child.stderr.take() {
            io.spawn(pipe(stderr, "stderr", lines));
        }
        let (report, status) = watch::channel(None);
        let (kill, killed) = mpsc::unbounded_channel();
        let pid = child.id();
        let stdin = child.stdin.take();
        io.spawn(supervise(child, killed, report));
        Self {
            pid,
            stdin: Arc::new(Mutex::new(stdin)),
            output: Mutex::new(output),
            status,
            kill,
        }
    }
}

async fn pipe(
    reader: impl AsyncRead + Unpin,
    stream: &'static str,
    lines: mpsc::UnboundedSender<Line>,
) {
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

async fn write(stdin: Arc<Mutex<Option<ChildStdin>>>, data: Vec<u8>) -> std::io::Result<()> {
    let mut stdin = stdin.lock().await;
    let pipe = stdin
        .as_mut()
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::BrokenPipe))?;
    pipe.write_all(&data).await?;
    pipe.flush().await
}

#[methods]
impl Proc {
    #[get]
    fn pid(&self) -> Option<u32> {
        self.pid
    }

    #[iterate(lines)]
    async fn line(&self) -> (Option<String>, Option<&'static str>) {
        self.output
            .lock()
            .await
            .recv()
            .await
            .map(|line| (line.text, line.stream))
            .unzip()
    }

    async fn write(
        &self,
        lua: Lua,
        data: &mlua::LuaString,
    ) -> mlua::Result<Result<bool, std::io::Error>> {
        let written = io::run(
            &lua,
            write(Arc::clone(&self.stdin), data.as_bytes().to_vec()),
        )
        .await?;
        Ok(written.map(|()| true))
    }

    async fn close(&self) {
        self.stdin.lock().await.take();
    }

    fn kill(&self) -> bool {
        self.kill.send(()).is_ok()
    }

    async fn wait(&self) -> mlua::Result<Exit> {
        let mut status = self.status.clone();
        Ok(status
            .wait_for(Option::is_some)
            .await
            .map_err(mlua::Error::external)?
            .unwrap_or(Exit {
                code: None,
                signal: None,
                success: false,
            }))
    }
}

#[derive(FromLua)]
struct SpawnOptions {
    cwd: Option<String>,
    env: Option<HashMap<String, String>>,
    stdio: Option<String>,
}

fn command(line: &[String], opts: SpawnOptions) -> mlua::Result<Command> {
    let Some((program, args)) = line.split_first() else {
        return Err(mlua::Error::runtime("proc.spawn needs a program"));
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
            return Err(mlua::Error::runtime(format!(
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

#[function(proc)]
fn spawn(
    lua: &Lua,
    argv: &[String],
    opts: SpawnOptions,
) -> mlua::Result<Result<Proc, std::io::Error>> {
    let mut command = command(argv, opts)?;
    let io = io::handle(lua)?;
    let entered = io.enter();
    let spawned = command.spawn();
    drop(entered);
    Ok(spawned.map(|child| Proc::start(&io, child)))
}
