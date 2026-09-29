use std::time::Duration;

use mlua::{LightUserData, Lua, Table};
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
    let names = uji_native::TYPES
        .iter()
        .map(|kind| format!("typedef struct {0} {0};", kind.name));
    let bodies = uji_native::TYPES
        .iter()
        .map(|kind| format!("struct {} {{ {} }};", kind.name, kind.fields));
    let cdef = names.chain(bodies).collect::<Vec<_>>().join("\n");
    let natives = uji_native::NATIVES
        .iter()
        .map(|native| {
            let entry = lua.create_table()?;
            entry.set("name", native.name)?;
            entry.set("signature", native.signature)?;
            entry.set(
                "address",
                LightUserData((native.address)().cast_mut().cast()),
            )?;
            Ok(entry)
        })
        .collect::<mlua::Result<Vec<Table>>>()?;
    let wrappers = uji_native::WRAPPERS
        .iter()
        .map(|wrapper| {
            lua.create_table_from([("place", wrapper.place), ("source", wrapper.source)])
        })
        .collect::<mlua::Result<Vec<_>>>()?;
    let table = lua.create_table()?;
    table.set("cdef", cdef)?;
    table.set("natives", natives)?;
    table.set("wrappers", wrappers)?;
    lua.globals().set("UJI_NATIVE", table)
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
