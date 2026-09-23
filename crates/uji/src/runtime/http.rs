use std::collections::BTreeMap;
use std::error::Error as _;
use std::time::Duration;

use futures_util::StreamExt as _;
use mlua::{IntoLua, Lua, Value};
use reqwest::header::HeaderMap;
use tokio::time::Instant;

use crate::api::http::{Fetch, Lines};

const MAX_BODY: usize = 8 * 1024 * 1024;

pub(crate) struct Response {
    status: u16,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum FetchError {
    #[error("request timed out")]
    Timeout,
    #[error("{}", cause(.0))]
    Transport(reqwest::Error),
    #[error("response body is over {MAX_BODY} bytes")]
    TooLarge,
    #[error("the server sent nothing for {}s", .0.as_secs())]
    Stalled(Duration),
    #[error("cancelled")]
    Cancelled,
}

impl From<reqwest::Error> for FetchError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::Timeout
        } else {
            Self::Transport(err)
        }
    }
}

fn cause(err: &reqwest::Error) -> String {
    std::iter::successors(err.source(), |&err| err.source())
        .last()
        .map_or_else(|| err.to_string(), |root| format!("{err}: {root}"))
}

impl IntoLua for Response {
    fn into_lua(self, lua: &Lua) -> mlua::Result<Value> {
        let table = lua.create_table()?;
        table.set("status", self.status)?;
        table.set("headers", self.headers)?;
        table.set("body", lua.create_string(&self.body)?)?;
        Ok(Value::Table(table))
    }
}

pub(crate) async fn fetch(
    client: &reqwest::Client,
    fetch: Fetch,
    on_line: impl Fn(String),
) -> Result<Response, FetchError> {
    let cancel = fetch.cancel.clone();
    tokio::select! {
        response = transfer(client, fetch, on_line) => response,
        () = cancel.cancelled() => Err(FetchError::Cancelled),
    }
}

async fn transfer(
    client: &reqwest::Client,
    fetch: Fetch,
    on_line: impl Fn(String),
) -> Result<Response, FetchError> {
    let mut request = client
        .request(fetch.method, fetch.url)
        .headers(fetch.headers);
    if let Some(timeout) = fetch.timeout {
        request = request.timeout(timeout);
    }
    if let Some(body) = fetch.body {
        request = request.body(body);
    }
    let response = request.send().await?;
    let status = response.status();
    let headers = joined(response.headers());
    let body = match fetch.lines {
        Some(lines) if status.is_success() => {
            stream(response, &lines, on_line).await?;
            Vec::new()
        }
        _ => read(response).await?,
    };
    Ok(Response {
        status: status.as_u16(),
        headers,
        body,
    })
}

async fn read(mut response: reqwest::Response) -> Result<Vec<u8>, FetchError> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if body.len().saturating_add(chunk.len()) > MAX_BODY {
            return Err(FetchError::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

async fn stream(
    response: reqwest::Response,
    lines: &Lines,
    on_line: impl Fn(String),
) -> Result<(), FetchError> {
    let mut chunks = response.bytes_stream();
    let mut pending = Vec::new();
    loop {
        let deadline = *lines.progress.borrow() + lines.idle;
        let Ok(next) = tokio::time::timeout_at(deadline, chunks.next()).await else {
            if *lines.progress.borrow() + lines.idle > Instant::now() {
                continue;
            }
            return Err(FetchError::Stalled(lines.idle));
        };
        let Some(chunk) = next else {
            if !pending.is_empty() {
                on_line(text(&pending));
            }
            return Ok(());
        };
        pending.extend_from_slice(&chunk?);
        while let Some(end) = pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = pending.drain(..=end).collect();
            on_line(text(&line));
        }
    }
}

fn text(line: &[u8]) -> String {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    String::from_utf8_lossy(line).into_owned()
}

fn joined(headers: &HeaderMap) -> BTreeMap<String, String> {
    headers
        .keys()
        .map(|name| {
            let values: Vec<_> = headers
                .get_all(name)
                .iter()
                .map(|value| String::from_utf8_lossy(value.as_bytes()))
                .collect();
            (name.as_str().to_owned(), values.join(", "))
        })
        .collect()
}
