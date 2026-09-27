use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use mlua::Lua;
use uji_macros::{FromLua, function, register};

use crate::kernel::{Restart, State};

#[function(os)]
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok())
        .unwrap_or_default()
}

#[function(os)]
fn env(name: String) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

#[function(os)]
fn cwd() -> mlua::Result<String> {
    std::env::current_dir()
        .map(|dir| dir.display().to_string())
        .map_err(mlua::Error::external)
}

#[function(os)]
fn home() -> Option<String> {
    std::env::home_dir().map(|dir| dir.display().to_string())
}

#[function(os)]
fn clock(lua: &Lua) -> mlua::Result<f64> {
    Ok(State::of(lua)?.started.elapsed().as_secs_f64())
}

#[derive(FromLua)]
struct RestartOptions {
    args: Vec<String>,
    #[lua(default)]
    roots: Vec<String>,
    carry: Option<String>,
}

#[function(os)]
fn restart(lua: &Lua, opts: RestartOptions) -> mlua::Result<()> {
    State::of_mut(lua)?.restart = Some(Restart {
        args: opts.args,
        roots: opts.roots.into_iter().map(PathBuf::from).collect(),
        carry: opts.carry,
    });
    Ok(())
}

#[function(os)]
fn exit(lua: &Lua, code: Option<u8>) -> mlua::Result<()> {
    State::of_mut(lua)?.exit = Some(code.unwrap_or(0));
    Ok(())
}

#[register(os)]
fn platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macos",
        "linux" => "linux",
        "windows" => "windows",
        _ => "other",
    }
}
