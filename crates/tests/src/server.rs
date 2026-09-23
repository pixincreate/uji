use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use serde_json::{Value, json};

type Handler = dyn Fn(&Request) -> Reply + Send + Sync;

#[derive(Clone)]
pub struct Request {
    pub path: String,
    pub headers: BTreeMap<String, String>,
    pub body: Value,
}

impl Request {
    pub fn has_tools(&self) -> bool {
        self.body["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty())
    }

    pub fn system(&self) -> &str {
        self.body["messages"][0]["content"]
            .as_str()
            .unwrap_or_default()
    }
}

pub enum Reply {
    Lines(Vec<String>),
    Status {
        code: u16,
        headers: Vec<(&'static str, String)>,
        body: String,
    },
    Drip {
        line: String,
        every: Duration,
    },
}

pub fn events(chunks: &[Value]) -> Reply {
    let mut lines: Vec<String> = chunks
        .iter()
        .map(|chunk| format!("data: {chunk}"))
        .collect();
    lines.push(String::from("data: [DONE]"));
    Reply::Lines(lines)
}

pub fn text(value: &str) -> Reply {
    events(&[json!({
        "choices": [{"index": 0, "delta": {"content": value}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 10, "completion_tokens": 2},
    })])
}

pub fn tool_calls(round: usize, calls: &[(&str, &str)]) -> Reply {
    let calls: Vec<Value> = calls
        .iter()
        .enumerate()
        .map(|(at, (name, arguments))| {
            json!({
                "index": at,
                "id": format!("r{round}c{at}"),
                "type": "function",
                "function": {"name": name, "arguments": arguments},
            })
        })
        .collect();
    events(&[
        json!({"choices": [{"index": 0, "delta": {"tool_calls": calls}, "finish_reason": null}]}),
        json!({
            "choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls"}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 2},
        }),
    ])
}

pub struct Server {
    pub url: String,
    incoming: Receiver<Request>,
    requests: RefCell<Vec<Request>>,
    dropped: Arc<AtomicUsize>,
}

impl Server {
    pub fn start(handler: impl Fn(&Request) -> Reply + Send + Sync + 'static) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let url = format!("http://{}", listener.local_addr()?);
        let (log, incoming) = mpsc::channel();
        let dropped = Arc::new(AtomicUsize::new(0));
        let handler: Arc<Handler> = Arc::new(handler);
        let lost = Arc::clone(&dropped);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let handler = Arc::clone(&handler);
                let log = log.clone();
                let lost = Arc::clone(&lost);
                std::thread::spawn(move || {
                    if serve(stream, handler.as_ref(), &log).is_err() {
                        lost.fetch_add(1, Ordering::SeqCst);
                    }
                });
            }
        });
        Ok(Self {
            url,
            incoming,
            requests: RefCell::default(),
            dropped,
        })
    }

    pub fn requests(&self) -> Vec<Request> {
        let mut requests = self.requests.borrow_mut();
        requests.extend(self.incoming.try_iter());
        requests.clone()
    }

    pub fn turns(&self) -> Vec<Request> {
        self.requests()
            .into_iter()
            .filter(Request::has_tools)
            .collect()
    }

    pub fn dropped(&self) -> usize {
        self.dropped.load(Ordering::SeqCst)
    }
}

fn serve(stream: TcpStream, handler: &Handler, log: &mpsc::Sender<Request>) -> io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let path = line
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_string();
    let mut headers = BTreeMap::new();
    loop {
        line.clear();
        reader.read_line(&mut line)?;
        let Some((name, value)) = line.trim_end().split_once(':') else {
            break;
        };
        headers.insert(name.to_ascii_lowercase(), value.trim().to_string());
    }
    let length = headers
        .get("content-length")
        .and_then(|length| length.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let request = Request {
        path,
        headers,
        body: serde_json::from_slice(&body).unwrap_or(Value::Null),
    };
    let reply = handler(&request);
    let _ = log.send(request);
    let mut stream = stream;
    match reply {
        Reply::Lines(lines) => {
            let body = lines.iter().fold(String::new(), |mut body, line| {
                let _ = write!(body, "{line}\n\n");
                body
            });
            let kind = [("content-type", String::from("text/event-stream"))];
            respond(&mut stream, 200, &kind, &body)
        }
        Reply::Status {
            code,
            headers,
            body,
        } => respond(&mut stream, code, &headers, &body),
        Reply::Drip { line, every } => drip(&mut stream, &line, every),
    }
}

fn respond(
    stream: &mut TcpStream,
    code: u16,
    headers: &[(&str, String)],
    body: &str,
) -> io::Result<()> {
    let extra = headers
        .iter()
        .fold(String::new(), |mut extra, (name, value)| {
            let _ = write!(extra, "{name}: {value}\r\n");
            extra
        });
    let head = format!(
        "HTTP/1.1 {code} X\r\nconnection: close\r\ncontent-length: {}\r\n{extra}\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())
}

fn drip(stream: &mut TcpStream, line: &str, every: Duration) -> io::Result<()> {
    stream.write_all(
        b"HTTP/1.1 200 X\r\nconnection: close\r\ncontent-type: text/event-stream\r\n\r\n",
    )?;
    loop {
        stream.write_all(format!("{line}\n\n").as_bytes())?;
        stream.flush()?;
        std::thread::sleep(every);
    }
}
