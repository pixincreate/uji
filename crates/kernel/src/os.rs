use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use uji_native::{Json, List, native};

use crate::context;
use crate::kernel::Restart;

#[native(os)]
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok())
        .unwrap_or_default()
}

#[native(os)]
fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

#[native(os, raise)]
fn cwd() -> std::io::Result<String> {
    std::env::current_dir().map(|dir| dir.display().to_string())
}

#[native(os)]
fn home() -> Option<String> {
    std::env::home_dir().map(|dir| dir.display().to_string())
}

#[native(os)]
fn clock() -> f64 {
    context::with(|context| context.started.elapsed().as_secs_f64()).unwrap_or_default()
}

#[derive(Deserialize)]
struct RestartOptions {
    #[serde(default)]
    args: List<String>,
    #[serde(default)]
    roots: List<String>,
    carry: Option<String>,
}

#[native(os)]
fn restart(opts: Json<RestartOptions>) {
    let Json(opts) = opts;
    let restart = Restart {
        args: opts.args.0,
        roots: opts.roots.0.into_iter().map(PathBuf::from).collect(),
        carry: opts.carry,
    };
    context::with(|context| context.restart = Some(restart));
}

#[native(os)]
fn exit(code: u8) {
    context::with(|context| context.exit = Some(code));
}
