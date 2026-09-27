use std::pin::pin;
use std::time::Duration;

use futures_util::future::{self, AbortHandle, Abortable, Either};
use mlua::{Function, IntoLuaMulti, Lua, MultiValue, Value, Variadic};
use uji_macros::{function, methods};

use crate::io;
use crate::kernel::State;

pub(crate) struct Task(AbortHandle);

#[methods]
impl Task {
    fn cancel(&self) {
        self.0.abort();
    }
}

pub(crate) fn start(lua: &Lua, function: &Function, args: impl IntoLuaMulti) -> mlua::Result<Task> {
    let call = function.call_async::<()>(args);
    let (handle, registration) = AbortHandle::new_pair();
    let owner = lua.clone();
    let scheduler = State::of(lua)?.scheduler.clone();
    scheduler
        .schedule(async move {
            match Abortable::new(call, registration).await {
                Ok(Err(err)) => State::report(&owner, &err.to_string()),
                Err(_) => {
                    let _ = owner.gc_collect();
                }
                Ok(Ok(())) => {}
            }
            finish(&owner);
        })
        .map_err(|_| mlua::Error::runtime("the executor has stopped"))?;
    State::of_mut(lua)?.pending += 1;
    Ok(Task(handle))
}

fn finish(lua: &Lua) {
    if let Ok(mut state) = State::of_mut(lua) {
        state.pending = state.pending.saturating_sub(1);
    }
}

#[function(task)]
fn spawn(lua: &Lua, function: &Function, args: MultiValue) -> mlua::Result<Task> {
    start(lua, function, args)
}

#[function(task)]
fn on_error(lua: &Lua, handler: Option<Function>) -> mlua::Result<()> {
    State::of_mut(lua)?.on_error = handler;
    Ok(())
}

#[function]
async fn sleep(lua: Lua, seconds: f64) -> mlua::Result<()> {
    let duration = Duration::try_from_secs_f64(seconds)
        .map_err(|_| mlua::Error::runtime("sleep needs a number of seconds"))?;
    io::sleep(&lua, duration)?.await;
    Ok(())
}

#[function(task)]
async fn race(functions: Variadic<Function>) -> mlua::Result<MultiValue> {
    if functions.is_empty() {
        return Err(mlua::Error::runtime("race needs at least one function"));
    }
    let calls = functions
        .iter()
        .map(|function| Box::pin(function.call_async::<MultiValue>(())));
    let (outcome, index, _) = future::select_all(calls).await;
    let mut values = outcome?;
    let position = i64::try_from(index.saturating_add(1)).unwrap_or(i64::MAX);
    values.push_front(Value::Integer(position));
    Ok(values)
}

#[function(task)]
async fn timeout(lua: Lua, seconds: f64, function: Function) -> mlua::Result<MultiValue> {
    let limit = Duration::try_from_secs_f64(seconds)
        .map_err(|_| mlua::Error::runtime("timeout needs a number of seconds"))?;
    let timer = io::sleep(&lua, limit)?;
    let call = function.call_async::<MultiValue>(());
    match future::select(pin!(call), pin!(timer)).await {
        Either::Left((values, _)) => {
            let mut values = values?;
            values.push_front(Value::Boolean(true));
            Ok(values)
        }
        Either::Right(((), _)) => Ok(MultiValue::from_vec(vec![Value::Boolean(false)])),
    }
}
