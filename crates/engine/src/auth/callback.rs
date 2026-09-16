use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

use super::AuthError;

const READ_TIMEOUT: Duration = Duration::from_secs(5);
const ACCEPT_POLL: Duration = Duration::from_millis(200);

pub struct Callback {
    listener: TcpListener,
    path: String,
}

impl Callback {
    pub fn bind(port: u16, path: &str) -> Result<Self, AuthError> {
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let listener = TcpListener::bind(addr).map_err(|err| AuthError::Callback {
            port,
            reason: err.to_string(),
        })?;
        listener
            .set_nonblocking(true)
            .map_err(|err| AuthError::Callback {
                port,
                reason: err.to_string(),
            })?;
        Ok(Self {
            listener,
            path: path.to_string(),
        })
    }

    pub fn wait(&self, timeout: Duration) -> Result<HashMap<String, String>, AuthError> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if let Some(params) = self.serve(stream) {
                        return Ok(params);
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(ACCEPT_POLL);
                }
                Err(err) => {
                    return Err(AuthError::Callback {
                        port: self.port(),
                        reason: err.to_string(),
                    });
                }
            }
        }
        Err(AuthError::TimedOut)
    }

    pub fn port(&self) -> u16 {
        self.listener
            .local_addr()
            .map_or(0, |addr: SocketAddr| addr.port())
    }

    fn serve(&self, mut stream: TcpStream) -> Option<HashMap<String, String>> {
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
        let mut line = String::new();
        if BufReader::new(&stream).read_line(&mut line).is_err() {
            return None;
        }
        let target = line.split_whitespace().nth(1)?;
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        if path != self.path {
            respond(&mut stream, "404 Not Found", "Unknown callback path.");
            return None;
        }
        let params = parse_query(query);
        if let Some(error) = params.get("error") {
            respond(&mut stream, "400 Bad Request", error);
            return None;
        }
        if !params.contains_key("code") {
            respond(
                &mut stream,
                "400 Bad Request",
                "Missing authorization code.",
            );
            return None;
        }
        respond(
            &mut stream,
            "200 OK",
            "Signed in. You can close this tab and return to uji.",
        );
        Some(params)
    }
}

fn respond(stream: &mut TcpStream, status: &str, message: &str) {
    let body = format!(
        "<!doctype html><meta charset=utf-8><title>uji</title>\
         <body style=\"font:16px system-ui;padding:3rem;color:#111\">{}</body>",
        html_escape(message)
    );
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn parse_query(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            let key = urlencoding::decode(key).ok()?.into_owned();
            let value = urlencoding::decode(value).ok()?.into_owned();
            Some((key, value))
        })
        .collect()
}
