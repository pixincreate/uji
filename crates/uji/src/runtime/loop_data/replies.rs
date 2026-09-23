use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use mlua::{IntoLua, IntoLuaMulti, Lua, Value};
use uji_core::llm::summary::{self, Compacted};
use uji_core::session::model::Message;

use super::LoopData;
use crate::api::fs::FsOp;
use crate::api::http::Fetch;
use crate::runtime::reply::{Json, Reply};
use crate::runtime::signal::Signal;
use crate::runtime::{fs, http};

struct Folded(Compacted);

impl IntoLua for Folded {
    fn into_lua(self, lua: &Lua) -> mlua::Result<Value> {
        let Compacted {
            messages,
            summary,
            usage,
            count,
        } = self.0;
        let table = lua.create_table()?;
        table.set("messages", Json(messages))?;
        table.set("summary", summary)?;
        table.set("usage", Json(usage))?;
        table.set("count", count)?;
        Ok(Value::Table(table))
    }
}

impl LoopData {
    pub(super) fn start_fetch(&self, id: u64, fetch: Fetch) {
        let client = Arc::clone(&self.inner.client);
        self.work.stream(|signals| async move {
            let lines = signals.clone();
            let on_line = move |line| {
                let _ = lines.send(Signal::Line { id, line });
            };
            let result = http::fetch(&client, fetch, on_line).await;
            let _ = signals.send(Signal::Reply(Reply::new(id, result)));
        });
    }

    pub(super) fn start_fs(&self, id: u64, op: FsOp) {
        let cwd = PathBuf::from(&self.app.messages().info().directory);
        let files = self.inner.api.access().borrow().files(cwd);
        self.work
            .spawn_blocking(move || Signal::Reply(fs::run(id, op, &files)));
    }

    pub(super) fn on_line(&mut self, id: u64, line: &str) {
        let Some(on_line) = self.inner.api.fetches().borrow().on_line(id) else {
            return;
        };
        match on_line.call::<Value>(line) {
            Ok(Value::Boolean(false)) => {}
            Ok(_) => self.inner.api.fetches().borrow().progressed(id),
            Err(err) => self.inner.notify(format!("http {id} on_line: {err}")),
        }
        self.dirty = true;
    }

    pub(super) fn start_route(&self, id: u64) {
        let llm = Arc::clone(&self.inner.llm);
        let client = Arc::clone(&self.inner.client);
        self.work.spawn(async move {
            let route = llm.route(&client).await.map(Json);
            Signal::Reply(Reply::new(id, route))
        });
    }

    pub(super) fn start_defer(&self, id: u64, after: Duration) {
        self.work.spawn(async move {
            tokio::time::sleep(after).await;
            Signal::Reply(Reply::new(id, Ok::<_, Infallible>(true)))
        });
    }

    pub(super) fn start_compact(&mut self, id: u64, messages: Vec<Message>) {
        let Some(budget) = self.budget() else {
            self.answer(id, Value::Nil);
            return;
        };
        let keep_recent = self.keep_recent();
        let llm = Arc::clone(&self.inner.llm);
        let model = self.inner.llm_model.clone();
        let client = Arc::clone(&self.inner.client);
        self.work.spawn(async move {
            let compacted =
                summary::compact(&client, llm.as_ref(), model, budget, keep_recent, messages)
                    .await
                    .map(Folded);
            Signal::Reply(Reply::new(id, Ok::<_, Infallible>(compacted)))
        });
    }

    pub(super) fn on_reply(&mut self, reply: Reply) {
        let id = reply.id;
        self.inner.api.fetches().borrow_mut().finish(id);
        match reply.into_args(&self.inner.lua) {
            Ok(args) => self.answer(id, args),
            Err(err) => self.inner.notify(format!("callback {id}: {err}")),
        }
    }

    pub(super) fn answer(&mut self, id: u64, args: impl IntoLuaMulti) {
        let Some(callback) = self.inner.api.callbacks().borrow_mut().take(id) else {
            return;
        };
        if let Err(err) = callback.call::<()>(args) {
            self.inner.notify(format!("callback {id}: {err}"));
        }
        self.dirty = true;
    }
}
