use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, Method, Url};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::{Mutex, mpsc, oneshot};
use uji_native::{Held, Json, native};

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

#[derive(Deserialize)]
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

#[derive(Serialize)]
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
    chunks: Mutex<Chunks>,
    _task: Abort,
}

#[native(net)]
async fn request(opts: Json<RequestOptions>) -> Result<Json<Response>, NetError> {
    let response = Request::new(opts.0)?.build().send().await?;
    let head = Head::of(&response);
    let body = response.text().await?;
    Ok(Json(Response { head, body }))
}

#[native(net)]
async fn open(opts: Json<RequestOptions>) -> Result<Held<Arc<Body>, Head>, NetError> {
    let request = Request::new(opts.0)?;
    let (head, answered) = oneshot::channel();
    let (chunks, receiver) = mpsc::channel(CHUNKS);
    let task = tokio::spawn(stream(request, head, chunks));
    let guard = Abort(task.abort_handle());
    let head = answered.await.unwrap_or(Err(NetError::Stopped))?;
    let body = Body {
        chunks: Mutex::new(Chunks {
            receiver,
            buffer: Vec::new(),
            done: false,
        }),
        _task: guard,
    };
    Ok(Held(Arc::new(body), head))
}

#[native(iterate = lines)]
async fn line(body: Arc<Body>, wait: Option<f64>) -> Result<Line, NetError> {
    let limit = seconds(wait, "wait")?;
    let mut chunks = body.chunks.lock().await;
    match chunks.ready()? {
        Ready::Line(line) => Ok(Line::Text(line)),
        Ready::End => Ok(Line::End),
        Ready::Wait => match limit {
            None => Ok(chunks.line().await?.into()),
            Some(limit) => match tokio::time::timeout(limit, chunks.line()).await {
                Ok(read) => Ok(read?.into()),
                Err(_) => Ok(Line::Late),
            },
        },
    }
}

#[native]
async fn read(body: Arc<Body>) -> Result<Vec<u8>, NetError> {
    body.chunks.lock().await.rest().await
}

pub(crate) struct Server {
    listener: Mutex<Option<Arc<TcpListener>>>,
}

#[derive(Serialize)]
struct Bound {
    port: u16,
}

pub(crate) struct Conn {
    reader: Mutex<BufReader<OwnedReadHalf>>,
    writer: Mutex<OwnedWriteHalf>,
}

#[native(net)]
async fn listen(port: Option<u16>) -> Result<Held<Arc<Server>, Bound>, NetError> {
    let listener = TcpListener::bind((LOOPBACK, port.unwrap_or(0))).await?;
    let port = listener.local_addr()?.port();
    let server = Server {
        listener: Mutex::new(Some(Arc::new(listener))),
    };
    Ok(Held(Arc::new(server), Bound { port }))
}

#[native]
async fn accept(server: Arc<Server>) -> Result<Held<Arc<Conn>>, NetError> {
    let listener = server
        .listener
        .lock()
        .await
        .clone()
        .ok_or(NetError::Closed)?;
    let (stream, _) = listener.accept().await?;
    let (reader, writer) = stream.into_split();
    Ok(Held::new(Arc::new(Conn {
        reader: Mutex::new(BufReader::new(reader)),
        writer: Mutex::new(writer),
    })))
}

#[native]
async fn close(server: Arc<Server>) {
    server.listener.lock().await.take();
}

async fn read_line(reader: &mut BufReader<OwnedReadHalf>) -> std::io::Result<Option<Vec<u8>>> {
    let mut line = String::new();
    if reader.read_line(&mut line).await? == 0 {
        return Ok(None);
    }
    Ok(Some(
        line.trim_end_matches(['\r', '\n']).as_bytes().to_vec(),
    ))
}

#[native]
async fn line(conn: Arc<Conn>, wait: Option<f64>) -> Result<Line, NetError> {
    let limit = seconds(wait, "wait")?;
    let mut reader = conn.reader.lock().await;
    match limit {
        None => Ok(read_line(&mut reader).await?.into()),
        Some(limit) => match tokio::time::timeout(limit, read_line(&mut reader)).await {
            Ok(read) => Ok(read?.into()),
            Err(_) => Ok(Line::Late),
        },
    }
}

#[native]
async fn read(conn: Arc<Conn>, count: usize) -> Result<Vec<u8>, NetError> {
    let mut buffer = vec![0; count];
    conn.reader.lock().await.read_exact(&mut buffer).await?;
    Ok(buffer)
}

#[native]
async fn write(conn: Arc<Conn>, data: Vec<u8>) -> Result<(), NetError> {
    let mut writer = conn.writer.lock().await;
    writer.write_all(&data).await?;
    writer.flush().await?;
    Ok(())
}

#[native]
async fn close(conn: Arc<Conn>) -> Result<(), NetError> {
    conn.writer.lock().await.shutdown().await?;
    Ok(())
}
