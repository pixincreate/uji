use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use uji_macros::{constant, function, options};

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
fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

#[function(os)]
fn cwd() -> mlua::Result<String> {
    Ok(std::env::current_dir()?.display().to_string())
}

#[function(os)]
fn home() -> Option<String> {
    std::env::home_dir().map(|dir| dir.display().to_string())
}

#[function(os)]
fn clock(state: &State) -> f64 {
    state.started.elapsed().as_secs_f64()
}

#[options]
struct RestartOptions {
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    roots: Vec<String>,
    carry: Option<String>,
}

#[function(os)]
fn restart(state: &mut State, opts: RestartOptions) {
    state.restart = Some(Restart {
        args: opts.args,
        roots: opts.roots.into_iter().map(PathBuf::from).collect(),
        carry: opts.carry,
    });
    state.wake();
}

#[function(os)]
fn exit(state: &mut State, code: Option<u8>) {
    state.exit = Some(code.unwrap_or(0));
    state.wake();
}

#[constant(os)]
fn platform() -> &'static str {
    let os = std::env::consts::OS;
    if matches!(os, "macos" | "linux" | "windows") {
        os
    } else {
        "other"
    }
}

#[constant(os)]
fn library() -> &'static str {
    std::env::consts::DLL_EXTENSION
}

#[constant(os)]
fn roots(state: &State) -> Vec<String> {
    state.roots.clone()
}

#[constant(os)]
fn carry(state: &State) -> Option<String> {
    state.carry.clone()
}
