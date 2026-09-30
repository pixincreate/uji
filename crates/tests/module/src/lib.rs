use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use uji_native::{Held, Json, native};

static DROPPED: AtomicI64 = AtomicI64::new(0);

#[native]
fn add(a: i64, b: i64) -> i64 {
    a + b
}

#[native]
fn greet(name: &str) -> String {
    format!("hello {name}")
}

#[native]
fn refuse(reason: &str) -> Result<(), String> {
    Err(reason.to_string())
}

#[native(raise)]
fn insist(reason: &str) -> Result<(), String> {
    Err(reason.to_string())
}

#[native]
async fn later(ms: u64) -> String {
    tokio::time::sleep(Duration::from_millis(ms)).await;
    format!("after {ms} ms")
}

#[derive(Deserialize)]
struct Options {
    #[serde(default = "one")]
    step: i64,
}

fn one() -> i64 {
    1
}

pub struct Counter {
    value: AtomicI64,
    step: i64,
}

impl Drop for Counter {
    fn drop(&mut self) {
        DROPPED.fetch_add(1, Ordering::SeqCst);
    }
}

#[derive(Serialize)]
struct Made {
    start: i64,
}

#[native]
fn counter(start: i64, opts: Json<Options>) -> Held<Arc<Counter>, Made> {
    let Json(opts) = opts;
    let counter = Counter {
        value: AtomicI64::new(start),
        step: opts.step,
    };
    Held(Arc::new(counter), Made { start })
}

#[native]
async fn eventually(start: i64, ms: u64) -> Held<Arc<Counter>, Made> {
    tokio::time::sleep(Duration::from_millis(ms)).await;
    let counter = Counter {
        value: AtomicI64::new(start),
        step: 1,
    };
    Held(Arc::new(counter), Made { start })
}

#[native]
fn bump(counter: &Arc<Counter>) -> i64 {
    counter.value.fetch_add(counter.step, Ordering::SeqCst) + counter.step
}

#[native]
async fn settle(counter: Arc<Counter>, ms: u64) -> Result<i64, String> {
    tokio::time::sleep(Duration::from_millis(ms)).await;
    let value = counter.value.load(Ordering::SeqCst);
    if value < 0 {
        return Err("the counter went below zero".to_string());
    }
    Ok(value)
}

#[native]
fn dropped() -> i64 {
    DROPPED.load(Ordering::SeqCst)
}

uji_native::module!();
