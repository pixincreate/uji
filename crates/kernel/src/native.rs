use std::time::Duration;

use mlua::{LightUserData, Lua};
use uji_native::abi::{self, Answer, Owned};
use uji_native::{Host, native};

use crate::context;
use crate::queue;

struct Kernel;

impl uji_native::Kernel for Kernel {
    fn reserve() -> u64 {
        context::with(|context| context.queue.reserve()).unwrap_or_default()
    }

    fn finish(token: u64, answer: Owned) {
        queue::finish(token, answer);
    }
}

static HOST: Host = Host::of::<Kernel>();

pub(crate) fn install(lua: &Lua) -> mlua::Result<()> {
    abi::install(context::start);
    let native = lua.create_table()?;
    native.set("cdef", uji_native::declarations())?;
    let manifest = uji_native::manifest().cast_mut().cast();
    native.set("manifest", LightUserData(manifest))?;
    lua.globals().set("UJI_NATIVE", native)
}

fn seconds(value: f64) -> Duration {
    Duration::try_from_secs_f64(value).unwrap_or_default()
}

#[native(kernel)]
fn host() -> *const Host {
    &raw const HOST
}

#[native(kernel)]
fn poll(timeout: Option<f64>) -> u64 {
    context::with(|context| context.queue.poll(timeout.map(seconds))).unwrap_or_default()
}

#[native(kernel)]
fn take(token: u64) -> *mut Answer {
    context::with(|context| context.queue.take(token))
        .flatten()
        .map_or(std::ptr::null_mut(), Owned::into_raw)
}

#[native(kernel)]
fn cancel(token: u64) {
    context::with(|context| context.queue.cancel(token));
}

#[native(kernel)]
fn stopping() -> bool {
    context::with(|context| context.exit.is_some() || context.restart.is_some()).unwrap_or(true)
}

#[native(kernel)]
fn report(message: &str) {
    context::with(|context| context.errors.push(message.to_string()));
}

#[native(kernel)]
async fn sleep(duration: f64) {
    tokio::time::sleep(seconds(duration)).await;
}
