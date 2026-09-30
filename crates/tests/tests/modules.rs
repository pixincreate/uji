use std::env::consts::DLL_EXTENSION;
use std::error::Error;
use std::path::PathBuf;
use std::process::Command;

use serde_json::{Value, json};
use uji_tests::Sandbox;

const MODULE: &str = "testmod";

fn library() -> Result<PathBuf, Box<dyn Error>> {
    let output = Command::new(env!("CARGO"))
        .args([
            "build",
            "--package",
            "uji-test-module",
            "--message-format",
            "json",
        ])
        .output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    String::from_utf8(output.stdout)?
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|message| message["reason"] == "compiler-artifact")
        .filter(|message| message["target"]["kind"] == json!(["cdylib"]))
        .flat_map(|message| message["filenames"].as_array().cloned().unwrap_or_default())
        .filter_map(|file| file.as_str().map(PathBuf::from))
        .find(|file| {
            file.extension()
                .is_some_and(|extension| extension == DLL_EXTENSION)
        })
        .ok_or_else(|| "cargo built no test module".into())
}

fn run(lua: &str) -> Result<Vec<Value>, Box<dyn Error>> {
    let sandbox = Sandbox::new("modules")?;
    let native = sandbox.root().join("cfg/native");
    std::fs::create_dir_all(&native)?;
    std::fs::copy(library()?, native.join(format!("{MODULE}.{DLL_EXTENSION}")))?;
    sandbox.probe(&format!("local module = require(\"{MODULE}\")\n{lua}"))
}

#[test]
fn a_native_module_is_called_like_a_lua_module() {
    let seen = run(r#"
        emit(module.add(40, 2))
        emit(module.greet("uji"))
        emit({ module.refuse("not today") })
        emit({ pcall(module.insist, "no") })
        emit((pcall(module.add, "forty", 2)))
    "#)
    .unwrap();
    assert_eq!(seen[0], 42);
    assert_eq!(seen[1], "hello uji");
    assert_eq!(
        seen[2],
        json!([null, "not today"]),
        "a Result error comes back as nil and the message"
    );
    assert_eq!(
        seen[3][0], false,
        "raise turns a Result error into a Lua error"
    );
    assert!(seen[3][1].as_str().unwrap_or_default().ends_with("no"));
    assert_eq!(
        seen[4], false,
        "an argument of the wrong type raises an error"
    );
}

#[test]
fn a_held_value_becomes_an_object_with_fields_and_methods() {
    let seen = run(r"
        local counter = module.counter(10, { step = 5 })
        emit(counter.start)
        emit(counter:bump())
        emit(counter:bump())
        emit(counter:settle(10))
        local plain = module.counter(0)
        emit(plain:bump())
        emit({ module.counter(-100):settle(1) })
    ")
    .unwrap();
    assert_eq!(seen[0], 10, "fields come from the second value of Held");
    assert_eq!(seen[1], 15);
    assert_eq!(seen[2], 20);
    assert_eq!(seen[3], 20, "an async method sees the same object");
    assert_eq!(seen[4], 1, "options left out take their defaults");
    assert_eq!(seen[5], json!([null, "the counter went below zero"]));
}

#[test]
fn async_calls_run_side_by_side_and_a_cancelled_one_is_dropped() {
    let seen = run(r"
        local started = uji.os.clock()
        local results = {}
        for index = 1, 3 do
            uji.task.spawn(function()
                results[index] = module.later(60 * index)
            end)
        end
        local dropped = uji.task.spawn(function()
            results[4] = module.later(2000)
        end)
        uji.sleep(0.4)
        dropped:cancel()
        emit({ results[1], results[2], results[3], results[4] == nil })
        emit(uji.os.clock() - started < 1)
    ")
    .unwrap();
    assert_eq!(
        seen[0],
        json!(["after 60 ms", "after 120 ms", "after 180 ms", true])
    );
    assert_eq!(
        seen[1], true,
        "the three calls overlapped instead of running one after another"
    );
}

#[test]
fn an_object_is_freed_once_lua_lets_go_of_it() {
    let seen = run(r"
        local before = module.dropped()
        local counter = module.counter(1)
        counter:bump()
        emit(module.dropped() - before)
        counter = nil
        collectgarbage()
        collectgarbage()
        emit(module.dropped() - before)
    ")
    .unwrap();
    assert_eq!(seen[0], 0, "the object lives while Lua holds it");
    assert_eq!(
        seen[1], 1,
        "the object is dropped in Rust after Lua collects it"
    );
}

#[test]
fn an_object_whose_task_was_cancelled_is_freed_too() {
    let seen = run(r"
        local before = module.dropped()
        local waiting = uji.task.spawn(function()
            module.eventually(1, 50)
        end)
        uji.sleep(0.01)
        waiting:cancel()
        uji.sleep(0.3)
        emit(module.dropped() - before)
    ")
    .unwrap();
    assert_eq!(
        seen[0], 1,
        "the object that arrived after the cancel was dropped, not leaked"
    );
}
