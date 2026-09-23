use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use mlua::{Function, Lua, LuaString, Table};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Method, Url};
use tokio::sync::watch;
use tokio::time::Instant;
use uji_agent::llm::{CancelToken, STREAM_IDLE};

use crate::api::Api;
use crate::api::bind::bind;
use crate::api::convert::seconds;
use crate::api::request::Request;

const TIMEOUT: Duration = Duration::from_secs(30);

pub struct Fetch {
    pub method: Method,
    pub url: Url,
    pub headers: HeaderMap,
    pub body: Option<Vec<u8>>,
    pub timeout: Option<Duration>,
    pub lines: Option<Lines>,
    pub cancel: CancelToken,
}

pub struct Lines {
    pub idle: Duration,
    pub progress: watch::Receiver<Instant>,
}

struct Live {
    cancel: CancelToken,
    on_line: Option<(Function, watch::Sender<Instant>)>,
}

#[derive(Default)]
pub struct Fetches {
    live: HashMap<u64, Live>,
}

impl Fetches {
    pub fn on_line(&self, id: u64) -> Option<Function> {
        self.live
            .get(&id)
            .and_then(|live| live.on_line.as_ref())
            .map(|(on_line, _)| on_line.clone())
    }

    pub fn progressed(&self, id: u64) {
        if let Some((_, progress)) = self.live.get(&id).and_then(|live| live.on_line.as_ref()) {
            progress.send_replace(Instant::now());
        }
    }

    pub fn finish(&mut self, id: u64) {
        self.live.remove(&id);
    }

    fn cancel(&self, id: u64) {
        if let Some(live) = self.live.get(&id) {
            live.cancel.cancel();
        }
    }
}

fn method(method: Option<String>) -> mlua::Result<Method> {
    method.map_or(Ok(Method::GET), |method| {
        Method::from_bytes(method.to_ascii_uppercase().as_bytes())
            .map_err(|_| mlua::Error::runtime(format!("invalid method {method}")))
    })
}

fn url(url: &str) -> mlua::Result<Url> {
    Url::parse(url).map_err(|err| mlua::Error::runtime(format!("invalid url {url}: {err}")))
}

fn headers(pairs: HashMap<String, String>) -> mlua::Result<HeaderMap> {
    pairs
        .into_iter()
        .map(|(name, value)| {
            let invalid = || mlua::Error::runtime(format!("invalid header {name}"));
            Ok((
                HeaderName::try_from(name.as_str()).map_err(|_| invalid())?,
                HeaderValue::try_from(value.as_str()).map_err(|_| invalid())?,
            ))
        })
        .collect()
}

pub(crate) fn request(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, |api, _, (opts, on_done): (Table, Function)| {
        let on_line = opts.get::<Option<Function>>("on_line")?;
        let timeout = seconds(&opts, "timeout")?.or(on_line.is_none().then_some(TIMEOUT));
        let cancel = CancelToken::new();
        let (on_line, lines) = match on_line {
            Some(on_line) => {
                let (progress, watch) = watch::channel(Instant::now());
                let lines = Lines {
                    idle: seconds(&opts, "idle")?.unwrap_or(STREAM_IDLE),
                    progress: watch,
                };
                (Some((on_line, progress)), Some(lines))
            }
            None => (None, None),
        };
        let fetch = Fetch {
            method: method(opts.get("method")?)?,
            url: url(&opts.get::<String>("url")?)?,
            headers: headers(opts.get::<Option<_>>("headers")?.unwrap_or_default())?,
            body: opts
                .get::<Option<LuaString>>("body")?
                .map(|body| body.as_bytes().to_vec()),
            timeout,
            lines,
            cancel: cancel.clone(),
        };
        let id = api.callbacks().borrow_mut().open(on_done);
        api.fetches()
            .borrow_mut()
            .live
            .insert(id, Live { cancel, on_line });
        api.request(Request::Fetch {
            id,
            fetch: Box::new(fetch),
        });
        Ok(id)
    })
}

pub(crate) fn cancel(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, |api, _, id: u64| {
        api.fetches().borrow().cancel(id);
        Ok(())
    })
}

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let http = lua.create_table()?;
    http.set("request", request(lua, api)?)?;
    http.set("cancel", cancel(lua, api)?)?;
    Ok(http)
}
