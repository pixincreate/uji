use std::cell::RefCell;
use std::rc::Rc;

use mlua::serde::SerializeOptions;
use mlua::{Function, Lua, LuaSerdeExt, Table, Value};
use tokio::sync::oneshot;
use uji_core::llm::{Answer, Call, Failure, LlmError, LlmResponse};

use super::LoopData;

type Outcome = Result<LlmResponse, LlmError>;

type Slot = Rc<RefCell<Option<oneshot::Sender<Outcome>>>>;

pub(crate) struct Live {
    reply: Slot,
    cancel: Option<Function>,
}

impl Live {
    fn waiting(&self) -> bool {
        self.reply
            .borrow()
            .as_ref()
            .is_some_and(|reply| !reply.is_closed())
    }
}

impl LoopData {
    pub(super) fn on_wire_call(&mut self, call: Call) {
        let Call {
            wire,
            request,
            reply,
        } = call;
        let reply: Slot = Rc::new(RefCell::new(Some(reply)));
        match self.start_wire(&wire, &request, &reply) {
            Ok(cancel) => self.calls.push(Live { reply, cancel }),
            Err(err) => answer(
                &reply,
                Err(LlmError::Provider(format!("{wire} wire: {err}"))),
            ),
        }
    }

    fn start_wire(
        &self,
        wire: &str,
        request: &serde_json::Value,
        reply: &Slot,
    ) -> mlua::Result<Option<Function>> {
        let lua = &self.inner.lua;
        let stream = self
            .inner
            .api
            .wires()
            .borrow()
            .get(wire)
            .cloned()
            .ok_or_else(|| mlua::Error::runtime(format!("no wire is registered as {wire}")))?;
        let options = SerializeOptions::new()
            .serialize_none_to_null(false)
            .serialize_unit_to_null(false);
        let request = lua.to_value_with(request, options)?;
        stream.call((request, replier(lua, reply)?))
    }

    pub(super) fn reap_calls(&mut self) {
        let (waiting, finished): (Vec<Live>, Vec<Live>) = std::mem::take(&mut self.calls)
            .into_iter()
            .partition(Live::waiting);
        self.calls = waiting;
        for live in finished {
            let abandoned = live.reply.borrow_mut().take().is_some();
            if let Some(cancel) = live.cancel.filter(|_| abandoned)
                && let Err(err) = cancel.call::<()>(())
            {
                self.inner.notify(format!("cancelling a wire call: {err}"));
            }
        }
    }
}

fn replier(lua: &Lua, reply: &Slot) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    let ignore = lua.create_function(|_, _: String| Ok(()))?;
    table.set("text", ignore.clone())?;
    table.set("reasoning", ignore)?;
    let done = Rc::clone(reply);
    table.set(
        "done",
        lua.create_function(move |lua, value: Value| {
            let outcome = lua
                .from_value::<Answer>(value)
                .map(LlmResponse::from)
                .map_err(|err| invalid("done", &err));
            answer(&done, outcome);
            Ok(())
        })?,
    )?;
    let fail = Rc::clone(reply);
    table.set(
        "fail",
        lua.create_function(move |lua, value: Value| {
            let failure = lua
                .from_value::<Failure>(value)
                .map_or_else(|err| invalid("fail", &err), LlmError::from);
            answer(&fail, Err(failure));
            Ok(())
        })?,
    )?;
    Ok(table)
}

fn invalid(call: &str, err: &mlua::Error) -> LlmError {
    LlmError::Provider(format!("the wire called {call} with {err}"))
}

fn answer(reply: &Slot, outcome: Outcome) {
    let taken = reply.borrow_mut().take();
    if let Some(reply) = taken {
        let _ = reply.send(outcome);
    }
}
