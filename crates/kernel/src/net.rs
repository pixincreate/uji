use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use futures_util::StreamExt;
use mlua::{BString, Lua, Table};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, Method, Url};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::{Mutex, mpsc, oneshot};
use uji_macros::{function, methods, options, value};

use crate::io::{Abort, Line};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_secs(150);
const IDLE: Duration = Duration::from_secs(120);
const CHUNKS: usize = 64;
const NEWLINE: u8 = b'\n';
const RETURN: u8 = b'\r';
const LOOPBACK: &str = "127.0.0.1";

static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .build()
        .unwrap_or_default()
});

pub(crate) fn warm() {
    LazyLock::force(&CLIENT);
}

#[derive(Debug, thiserror::Error)]
enum NetError {
    #[error("{0}")]
    Request(#[from] reqwest::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Invalid(String),
    #[error("no data arrived for {0} seconds")]
    Idle(u64),
    #[error("the request stopped before it answered")]
    Stopped,
    #[error("the server is closed")]
    Closed,
}

fn seconds(value: Option<f64>, key: &str) -> Result<Option<Duration>, NetError> {
    value
        .map(|value| {
            Duration::try_from_secs_f64(value)
                .map_err(|_| NetError::Invalid(format!("{key} needs a number of seconds")))
        })
        .transpose()
}

#[options]
struct RequestOptions {
    url: String,
    method: Option<String>,
    #[serde(default)]
    headers: HashMap<String, String>,
    body: Option<String>,
    timeout: Option<f64>,
    idle: Option<f64>,
}

struct Request {
    method: Method,
    url: Url,
    headers: HeaderMap,
    body: Option<String>,
    timeout: Option<Duration>,
    idle: Duration,
}

impl Request {
    fn new(opts: RequestOptions) -> Result<Self, NetError> {
        let method = opts.method.map_or(Ok(Method::GET), |method| {
            Method::from_bytes(method.to_ascii_uppercase().as_bytes())
                .map_err(|_| NetError::Invalid(format!("invalid method {method}")))
        })?;
        let url = Url::parse(&opts.url)
            .map_err(|err| NetError::Invalid(format!("invalid url {}: {err}", opts.url)))?;
        let headers = opts
            .headers
            .into_iter()
            .map(|(name, value)| {
                let invalid = || NetError::Invalid(format!("invalid header {name}"));
                Ok((
                    HeaderName::try_from(name.as_str()).map_err(|_| invalid())?,
                    HeaderValue::try_from(value.as_str()).map_err(|_| invalid())?,
                ))
            })
            .collect::<Result<HeaderMap, NetError>>()?;
        Ok(Self {
            method,
            url,
            headers,
            body: opts.body,
            timeout: seconds(opts.timeout, "timeout")?,
            idle: seconds(opts.idle, "idle")?.unwrap_or(IDLE),
        })
    }

    fn build(self) -> reqwest::RequestBuilder {
        let mut builder = CLIENT.request(self.method, self.url).headers(self.headers);
        if let Some(body) = self.body {
            builder = builder.body(body);
        }
        if let Some(timeout) = self.timeout {
            builder = builder.timeout(timeout);
        }
        builder
    }
}

#[value]
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

#[value]
struct Response {
    #[serde(flatten)]
    head: Head,
    body: String,
}

async fn stream(
    request: Request,
    head: oneshot::Sender<Result<Head, NetError>>,
    chunks: mpsc::Sender<Result<Vec<u8>, NetError>>,
) {
    let idle = request.idle;
    let response = match request.build().send().await {
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
            Ok(Some(chunk)) => chunk.map(Vec::from).map_err(NetError::from),
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

#[methods]
impl Body {
    #[get]
    fn status(&self) -> u16 {
        self.head.status
    }

    #[get]
    fn headers(&self, lua: &Lua) -> mlua::Result<Table> {
        lua.create_table_from(
            self.head
                .headers
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str())),
        )
    }

    #[iterate(lines)]
    async fn line(&self, wait: Option<f64>) -> Result<Line, NetError> {
        let limit = seconds(wait, "wait")?;
        let mut chunks = self.chunks.lock().await;
        match chunks.ready()? {
            Ready::Line(line) => Ok(Line::Text(line.into())),
            Ready::End => Ok(Line::End),
            Ready::Wait => Line::within(limit, chunks.line()).await,
        }
    }

    async fn read(&self) -> Result<BString, NetError> {
        self.chunks.lock().await.rest().await.map(BString::from)
    }
}

#[function(net)]
async fn request(opts: RequestOptions) -> Result<Response, NetError> {
    let response = Request::new(opts)?.build().send().await?;
    let head = Head::of(&response);
    let body = response.text().await?;
    Ok(Response { head, body })
}

#[function(net)]
async fn open(opts: RequestOptions) -> Result<Body, NetError> {
    let request = Request::new(opts)?;
    let (head, answered) = oneshot::channel();
    let (chunks, receiver) = mpsc::channel(CHUNKS);
    let task = tokio::spawn(stream(request, head, chunks));
    let guard = Abort(task.abort_handle());
    let head = answered.await.unwrap_or(Err(NetError::Stopped))?;
    Ok(Body {
        head,
        chunks: Mutex::new(Chunks {
            receiver,
            buffer: Vec::new(),
            done: false,
        }),
        _task: guard,
    })
}

pub(crate) struct Server {
    port: u16,
    listener: Mutex<Option<Arc<TcpListener>>>,
}

#[methods]
impl Server {
    #[get]
    fn port(&self) -> u16 {
        self.port
    }

    async fn accept(&self) -> Result<Conn, NetError> {
        let listener = self.listener.lock().await.clone().ok_or(NetError::Closed)?;
        let (stream, _) = listener.accept().await?;
        let (reader, writer) = stream.into_split();
        Ok(Conn {
            reader: Mutex::new(BufReader::new(reader)),
            writer: Mutex::new(writer),
        })
    }

    async fn close(&self) {
        self.listener.lock().await.take();
    }
}

#[function(net)]
async fn listen(port: Option<u16>) -> Result<Server, NetError> {
    let listener = TcpListener::bind((LOOPBACK, port.unwrap_or(0))).await?;
    Ok(Server {
        port: listener.local_addr()?.port(),
        listener: Mutex::new(Some(Arc::new(listener))),
    })
}

pub(crate) struct Conn {
    reader: Mutex<BufReader<OwnedReadHalf>>,
    writer: Mutex<OwnedWriteHalf>,
}

async fn read_line(reader: &mut BufReader<OwnedReadHalf>) -> std::io::Result<Option<Vec<u8>>> {
    let mut line = String::new();
    if reader.read_line(&mut line).await? == 0 {
        return Ok(None);
    }
    line.truncate(line.trim_end_matches(['\r', '\n']).len());
    Ok(Some(line.into_bytes()))
}

#[methods]
impl Conn {
    async fn line(&self, wait: Option<f64>) -> Result<Line, NetError> {
        let limit = seconds(wait, "wait")?;
        let mut reader = self.reader.lock().await;
        Ok(Line::within(limit, read_line(&mut reader)).await?)
    }

    async fn read(&self, count: usize) -> Result<BString, NetError> {
        let mut buffer = vec![0; count];
        self.reader.lock().await.read_exact(&mut buffer).await?;
        Ok(buffer.into())
    }

    async fn write(&self, data: &[u8]) -> Result<(), NetError> {
        let mut writer = self.writer.lock().await;
        writer.write_all(data).await?;
        writer.flush().await?;
        Ok(())
    }

    async fn close(&self) -> Result<(), NetError> {
        self.writer.lock().await.shutdown().await?;
        Ok(())
    }
}
