use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use std::pin::pin;

use futures_util::StreamExt;
use futures_util::future::{self, Either};
use mlua::{AnyUserData, BString, Function, Lua};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, Method, Url};
use serde::Serialize;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::{Mutex, mpsc, oneshot};
use uji_macros::{FromLua, IntoLua, function, methods};

use crate::io::{self, Abort};
use crate::kernel::State;

const IDLE: Duration = Duration::from_secs(120);
const CHUNKS: usize = 64;
const NEWLINE: u8 = b'\n';
const RETURN: u8 = b'\r';
const LOOPBACK: &str = "127.0.0.1";

#[derive(Debug, thiserror::Error)]
enum NetError {
    #[error("{0}")]
    Request(#[from] reqwest::Error),
    #[error("no data arrived for {0} seconds")]
    Idle(u64),
    #[error("the request stopped before it answered")]
    Stopped,
}

struct Request {
    method: Method,
    url: Url,
    headers: HeaderMap,
    body: Option<Vec<u8>>,
    timeout: Option<Duration>,
    idle: Duration,
}

#[derive(FromLua)]
struct RequestOptions {
    url: String,
    method: Option<String>,
    #[lua(default)]
    headers: HashMap<String, String>,
    body: Option<BString>,
    timeout: Option<f64>,
    idle: Option<f64>,
}

fn seconds(value: Option<f64>, key: &str) -> mlua::Result<Option<Duration>> {
    value
        .map(|value| {
            Duration::try_from_secs_f64(value)
                .map_err(|_| mlua::Error::runtime(format!("{key} needs a number of seconds")))
        })
        .transpose()
}

impl Request {
    fn new(opts: RequestOptions) -> mlua::Result<Self> {
        let method = opts.method.map_or(Ok(Method::GET), |method| {
            Method::from_bytes(method.to_ascii_uppercase().as_bytes())
                .map_err(|_| mlua::Error::runtime(format!("invalid method {method}")))
        })?;
        let url = Url::parse(&opts.url)
            .map_err(|err| mlua::Error::runtime(format!("invalid url {}: {err}", opts.url)))?;
        let headers = opts
            .headers
            .into_iter()
            .map(|(name, value)| {
                let invalid = || mlua::Error::runtime(format!("invalid header {name}"));
                Ok((
                    HeaderName::try_from(name.as_str()).map_err(|_| invalid())?,
                    HeaderValue::try_from(value.as_str()).map_err(|_| invalid())?,
                ))
            })
            .collect::<mlua::Result<HeaderMap>>()?;
        Ok(Self {
            method,
            url,
            headers,
            body: opts.body.map(Vec::from),
            timeout: seconds(opts.timeout, "timeout")?,
            idle: seconds(opts.idle, "idle")?.unwrap_or(IDLE),
        })
    }

    fn build(self, client: &Client) -> reqwest::RequestBuilder {
        let mut builder = client.request(self.method, self.url).headers(self.headers);
        if let Some(body) = self.body {
            builder = builder.body(body);
        }
        if let Some(timeout) = self.timeout {
            builder = builder.timeout(timeout);
        }
        builder
    }
}

#[derive(Serialize)]
struct Head {
    status: u16,
    headers: HashMap<String, String>,
}

impl Head {
    fn of(response: &reqwest::Response) -> Self {
        Self {
            status: response.status().as_u16(),
            headers: response
                .headers()
                .iter()
                .filter_map(|(name, value)| {
                    value
                        .to_str()
                        .ok()
                        .map(|value| (name.as_str().to_string(), value.to_string()))
                })
                .collect(),
        }
    }
}

#[derive(Serialize, IntoLua)]
struct Response {
    #[serde(flatten)]
    head: Head,
    body: String,
}

async fn fetch(client: Client, request: Request) -> Result<Response, NetError> {
    let response = request.build(&client).send().await?;
    let head = Head::of(&response);
    let body = response.text().await?;
    Ok(Response { head, body })
}

async fn stream(
    client: Client,
    request: Request,
    head: oneshot::Sender<Result<Head, NetError>>,
    chunks: mpsc::Sender<Result<Vec<u8>, NetError>>,
) {
    let idle = request.idle;
    let response = match request.build(&client).send().await {
        Ok(response) => response,
        Err(err) => {
            drop(head.send(Err(err.into())));
            return;
        }
    };
    if head.send(Ok(Head::of(&response))).is_err() {
        return;
    }
    let mut body = response.bytes_stream();
    loop {
        let next = match tokio::time::timeout(idle, body.next()).await {
            Ok(Some(chunk)) => chunk.map(|chunk| chunk.to_vec()).map_err(NetError::from),
            Ok(None) => return,
            Err(_) => Err(NetError::Idle(idle.as_secs())),
        };
        let failed = next.is_err();
        if chunks.send(next).await.is_err() || failed {
            return;
        }
    }
}

struct Chunks {
    receiver: mpsc::Receiver<Result<Vec<u8>, NetError>>,
    buffer: Vec<u8>,
    done: bool,
}

enum Ready {
    Line(Vec<u8>),
    End,
    Wait,
}

impl Chunks {
    fn split(&mut self) -> Option<Vec<u8>> {
        let at = self.buffer.iter().position(|byte| *byte == NEWLINE)?;
        let mut line: Vec<u8> = self.buffer.drain(..=at).collect();
        line.pop();
        if line.last() == Some(&RETURN) {
            line.pop();
        }
        Some(line)
    }

    fn remainder(&mut self) -> Option<Vec<u8>> {
        (!self.buffer.is_empty()).then(|| std::mem::take(&mut self.buffer))
    }

    fn ready(&mut self) -> Result<Ready, NetError> {
        loop {
            if let Some(line) = self.split() {
                return Ok(Ready::Line(line));
            }
            if self.done {
                return Ok(self.remainder().map_or(Ready::End, Ready::Line));
            }
            match self.receiver.try_recv() {
                Ok(chunk) => self.buffer.extend(chunk?),
                Err(TryRecvError::Empty) => return Ok(Ready::Wait),
                Err(TryRecvError::Disconnected) => self.done = true,
            }
        }
    }

    async fn line(&mut self) -> Result<Option<Vec<u8>>, NetError> {
        loop {
            if let Some(line) = self.split() {
                return Ok(Some(line));
            }
            if self.done {
                return Ok(self.remainder());
            }
            self.pull().await?;
        }
    }

    async fn rest(&mut self) -> Result<Vec<u8>, NetError> {
        while !self.done {
            self.pull().await?;
        }
        Ok(std::mem::take(&mut self.buffer))
    }

    async fn pull(&mut self) -> Result<(), NetError> {
        match self.receiver.recv().await {
            Some(chunk) => self.buffer.extend(chunk?),
            None => self.done = true,
        }
        Ok(())
    }
}

pub(crate) struct Body {
    head: Head,
    chunks: Mutex<Chunks>,
    _task: Abort,
}

impl Body {
    async fn next_line(
        &self,
        lua: &Lua,
        limit: Option<Duration>,
    ) -> mlua::Result<Result<io::Line, NetError>> {
        let mut chunks = self.chunks.lock().await;
        let read = match chunks.ready() {
            Ok(Ready::Line(line)) => Ok(Some(line)),
            Ok(Ready::End) => Ok(None),
            Err(err) => Err(err),
            Ok(Ready::Wait) => match limit {
                None => chunks.line().await,
                Some(limit) => {
                    let timer = io::sleep(lua, limit)?;
                    match future::select(pin!(chunks.line()), pin!(timer)).await {
                        Either::Left((read, _)) => read,
                        Either::Right(((), _)) => return Ok(Ok(io::Line::Late)),
                    }
                }
            },
        };
        Ok(read.map(io::Line::from))
    }
}

#[methods]
impl Body {
    #[get]
    fn status(&self) -> u16 {
        self.head.status
    }

    #[get]
    fn headers(&self) -> HashMap<String, String> {
        self.head.headers.clone()
    }

    async fn line(&self, lua: Lua, wait: Option<f64>) -> mlua::Result<Result<io::Line, NetError>> {
        self.next_line(&lua, io::limit(wait)?).await
    }

    fn lines(lua: &Lua, body: AnyUserData) -> mlua::Result<Function> {
        lua.create_async_function(move |lua, ()| {
            let body = body.clone();
            async move { io::settle(&lua, body.borrow::<Body>()?.next_line(&lua, None).await?) }
        })
    }

    async fn read(&self) -> Result<BString, NetError> {
        self.chunks.lock().await.rest().await.map(BString::from)
    }
}

async fn connect(lua: &Lua, request: Request) -> mlua::Result<Result<Body, NetError>> {
    let io = io::handle(lua)?;
    let client = State::of(lua)?.client();
    let (head, answered) = oneshot::channel();
    let (chunks, receiver) = mpsc::channel(CHUNKS);
    let task = io.spawn(stream(client, request, head, chunks));
    let guard = Abort(task.abort_handle());
    let answer = answered.await.unwrap_or(Err(NetError::Stopped));
    Ok(answer.map(|head| Body {
        head,
        chunks: Mutex::new(Chunks {
            receiver,
            buffer: Vec::new(),
            done: false,
        }),
        _task: guard,
    }))
}

pub(crate) struct Server {
    port: u16,
    listener: Mutex<Option<Arc<TcpListener>>>,
}

pub(crate) struct Conn {
    reader: Arc<Mutex<BufReader<OwnedReadHalf>>>,
    writer: Arc<Mutex<OwnedWriteHalf>>,
}

async fn read_line(
    reader: Arc<Mutex<BufReader<OwnedReadHalf>>>,
) -> std::io::Result<Option<String>> {
    let mut line = String::new();
    if reader.lock().await.read_line(&mut line).await? == 0 {
        return Ok(None);
    }
    Ok(Some(line.trim_end_matches(['\r', '\n']).to_string()))
}

async fn read_exact(
    reader: Arc<Mutex<BufReader<OwnedReadHalf>>>,
    count: usize,
) -> std::io::Result<Vec<u8>> {
    let mut buffer = vec![0; count];
    reader.lock().await.read_exact(&mut buffer).await?;
    Ok(buffer)
}

async fn write_all(writer: Arc<Mutex<OwnedWriteHalf>>, data: Vec<u8>) -> std::io::Result<()> {
    let mut writer = writer.lock().await;
    writer.write_all(&data).await?;
    writer.flush().await
}

async fn shutdown(writer: Arc<Mutex<OwnedWriteHalf>>) -> std::io::Result<()> {
    writer.lock().await.shutdown().await
}

#[methods]
impl Server {
    #[get]
    fn port(&self) -> u16 {
        self.port
    }

    async fn accept(&self, lua: Lua) -> mlua::Result<Result<Conn, std::io::Error>> {
        let Some(listener) = self.listener.lock().await.clone() else {
            return Ok(Err(std::io::Error::other("the server is closed")));
        };
        let accepted = io::run(&lua, async move { listener.accept().await }).await?;
        Ok(accepted.map(|(stream, _)| {
            let (reader, writer) = stream.into_split();
            Conn {
                reader: Arc::new(Mutex::new(BufReader::new(reader))),
                writer: Arc::new(Mutex::new(writer)),
            }
        }))
    }

    async fn close(&self) {
        self.listener.lock().await.take();
    }
}

#[methods]
impl Conn {
    async fn line(
        &self,
        lua: Lua,
        wait: Option<f64>,
    ) -> mlua::Result<Result<io::Line, std::io::Error>> {
        let limit = io::limit(wait)?;
        let reader = Arc::clone(&self.reader);
        let read = io::run(&lua, async move {
            match limit {
                Some(limit) => tokio::time::timeout(limit, read_line(reader)).await.ok(),
                None => Some(read_line(reader).await),
            }
        })
        .await?;
        Ok(read.map_or(Ok(io::Line::Late), |read| read.map(io::Line::from)))
    }

    async fn read(&self, lua: Lua, count: usize) -> mlua::Result<Result<BString, std::io::Error>> {
        let read = io::run(&lua, read_exact(Arc::clone(&self.reader), count)).await?;
        Ok(read.map(BString::from))
    }

    async fn write(
        &self,
        lua: Lua,
        data: &mlua::LuaString,
    ) -> mlua::Result<Result<bool, std::io::Error>> {
        let bytes = data.as_bytes().to_vec();
        let written = io::run(&lua, write_all(Arc::clone(&self.writer), bytes)).await?;
        Ok(written.map(|()| true))
    }

    async fn close(&self, lua: Lua) -> mlua::Result<Result<bool, std::io::Error>> {
        let closed = io::run(&lua, shutdown(Arc::clone(&self.writer))).await?;
        Ok(closed.map(|()| true))
    }
}

#[function(net)]
async fn request(lua: Lua, opts: RequestOptions) -> mlua::Result<Result<Response, NetError>> {
    let request = Request::new(opts)?;
    let client = State::of(&lua)?.client();
    io::run(&lua, fetch(client, request)).await
}

#[function(net)]
async fn open(lua: Lua, opts: RequestOptions) -> mlua::Result<Result<Body, NetError>> {
    connect(&lua, Request::new(opts)?).await
}

#[function(net)]
async fn listen(lua: Lua, port: Option<u16>) -> mlua::Result<Result<Server, std::io::Error>> {
    let bound = io::run(&lua, TcpListener::bind((LOOPBACK, port.unwrap_or(0)))).await?;
    Ok(bound.and_then(|listener| {
        Ok(Server {
            port: listener.local_addr()?.port(),
            listener: Mutex::new(Some(Arc::new(listener))),
        })
    }))
}
