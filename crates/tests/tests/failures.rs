use serde_json::{Value, json};
use uji_tests::probe;

fn all_false(seen: &[Value]) -> bool {
    seen.iter().all(|value| *value == json!(false))
}

#[test]
fn arguments_of_the_wrong_type_raise_an_error() {
    let seen = probe(
        r#"
        local sys = require("uji.sys")
        emit((pcall(sys.fs.read)))
        emit((pcall(sys.base64.encode, {})))
        emit((pcall(sys.sleep, "soon")))
        emit((pcall(sys.sleep, -1)))
        emit((pcall(sys.sleep, 0 / 0)))
        emit((pcall(sys.task.timeout, -1, function() end)))
        emit((pcall(sys.task.race)))
        emit((pcall(sys.proc.spawn, "not a list")))
        "#,
    )
    .unwrap();
    assert_eq!(seen.len(), 8);
    assert!(all_false(&seen), "every call raised: {seen:?}");
}

#[test]
fn expected_failures_come_back_as_nil_and_a_message() {
    let seen = probe(
        r#"
        local sys = require("uji.sys")
        local missing = probe.work .. "/missing"
        local function check(label, value, message)
            emit({ label, value == nil, type(message) == "string" and #message > 0 })
        end
        check("read", sys.fs.read(missing))
        check("list", sys.fs.list(missing))
        check("stat", sys.fs.stat(missing))
        check("remove", sys.fs.remove(missing))
        check("rename", sys.fs.rename(missing, missing .. "2"))
        check("spawn", sys.proc.spawn({ "uji-no-such-program" }))
        check("empty argv", sys.proc.spawn({}))
        check("url", sys.net.request({ url = "not a url" }))
        check("method", sys.net.request({ url = "http://127.0.0.1:9", method = "NOT A METHOD" }))
        check("regex", sys.regex("("))
        check("glob", sys.glob("a[b"))
        check("database", sys.db.open(missing .. "/nested/uji.db"))
        "#,
    )
    .unwrap();
    assert_eq!(seen.len(), 12);
    for row in &seen {
        assert_eq!(row[1], true, "{} gave a value", row[0]);
        assert_eq!(row[2], true, "{} gave no message", row[0]);
    }
}

#[test]
fn bad_input_to_a_codec_raises() {
    let seen = probe(
        r#"
        local sys = require("uji.sys")
        emit((pcall(sys.base64.decode, "@@@")))
        emit((pcall(sys.json.decode, "{")))
        emit((pcall(sys.json.encode, { f = function() end })))
        emit((pcall(sys.toml.decode, "= nope")))
        emit((pcall(sys.toml.encode, "not a table")))
        emit((pcall(sys.markdown)))
        "#,
    )
    .unwrap();
    assert_eq!(seen.len(), 6);
    assert!(all_false(&seen), "every call raised: {seen:?}");
}

#[test]
fn an_error_message_comes_without_the_stack_trace() {
    let seen = probe(
        r#"
        local sys = require("uji.sys")
        local _, decoded = pcall(sys.base64.decode, "@@@")
        emit(sys.message(decoded))
        local _, plain = pcall(error, "plain", 0)
        emit(sys.message(plain))
        emit(sys.message(setmetatable({}, { __tostring = function() return "custom" end })))
        emit(sys.message("bad \255 byte"))
        local _, raced = pcall(sys.task.race, function() error("from a racer", 0) end)
        emit(sys.message(raced))
        "#,
    )
    .unwrap();
    assert_eq!(seen[0], "Invalid symbol 64, offset 0.");
    assert_eq!(seen[1], "plain");
    assert_eq!(seen[2], "custom");
    assert_eq!(seen[3], "bad \u{FFFD} byte");
    assert_eq!(seen[4], "from a racer");
}

#[test]
fn a_failing_task_is_reported_and_the_others_keep_running() {
    let seen = probe(
        r#"
        local sys = require("uji.sys")
        local reported = {}
        sys.task.on_error(function(message)
            reported[#reported + 1] = message
        end)
        local survived = false
        sys.task.spawn(function()
            error("task boom", 0)
        end)
        sys.task.spawn(function()
            sys.sleep(0.01)
            survived = true
        end)
        sys.sleep(0.05)
        emit(reported[1], survived)
        "#,
    )
    .unwrap();
    assert_eq!(seen[0], "task boom", "the report has no stack trace");
    assert_eq!(seen[1], true);
}

#[test]
fn race_and_timeout_raise_the_original_error_and_stop_the_rest() {
    let seen = probe(
        r#"
        local sys = require("uji.sys")
        local raced, race_error = pcall(uji.task.race, function()
            error({ code = 7 })
        end, function()
            sys.sleep(1)
        end)
        emit(raced, type(race_error) == "table" and race_error.code)
        local timed, timeout_error = pcall(uji.task.timeout, 1, function()
            error({ code = 8 })
        end)
        emit(timed, type(timeout_error) == "table" and timeout_error.code)
        emit((uji.task.timeout(0.01, function()
            sys.sleep(1)
        end)))
        local loser = false
        uji.task.race(function()
            return "fast"
        end, function()
            sys.sleep(0.02)
            loser = true
        end)
        sys.sleep(0.05)
        emit(loser)
        "#,
    )
    .unwrap();
    assert_eq!(seen[..2], [json!(false), json!(7)]);
    assert_eq!(seen[2..4], [json!(false), json!(8)]);
    assert_eq!(seen[4], false, "the timeout ran out");
    assert_eq!(seen[5], false, "the losing racer was stopped");
}

#[test]
fn a_cancelled_task_stops_where_it_waits() {
    let seen = probe(
        r#"
        local sys = require("uji.sys")
        local reached = false
        local waiting = sys.task.spawn(function()
            sys.sleep(0.02)
            reached = true
        end)
        waiting:cancel()
        waiting:cancel()
        local done = sys.task.spawn(function() end)
        sys.sleep(0.05)
        done:cancel()
        emit(reached)
        "#,
    )
    .unwrap();
    assert_eq!(seen[0], false);
}

#[test]
fn a_promise_settles_once_and_keeps_every_value() {
    let seen = probe(
        r##"
        local sys = require("uji.sys")
        local promise = sys.promise()
        emit(promise.settled)
        emit(promise:resolve(1, nil, 3))
        emit(promise:resolve("again"))
        emit(promise.settled)
        local first, second, third = promise:await()
        emit(select("#", promise:await()), first, second == nil, third)
        local later = sys.promise()
        local got
        sys.task.spawn(function()
            got = later:await()
        end)
        sys.sleep(0)
        later:resolve("woken")
        sys.sleep(0)
        emit(got)
        "##,
    )
    .unwrap();
    assert_eq!(
        seen,
        [
            json!(false),
            json!(true),
            json!(false),
            json!(true),
            json!(3),
            json!(1),
            json!(true),
            json!(3),
            json!("woken"),
        ]
    );
}

#[test]
fn a_failed_transaction_rolls_back_and_raises_the_original_error() {
    let seen = probe(
        r#"
        local sys = require("uji.sys")
        local db = assert(sys.db.open(probe.work .. "/failures.db"))
        db:exec("CREATE TABLE items (name TEXT)")
        local function count()
            return #db:query("SELECT name FROM items")
        end
        local ok, err = pcall(db.transaction, db, function()
            db:exec("INSERT INTO items (name) VALUES (?)", { "dropped" })
            error({ code = 9 })
        end)
        emit(ok, type(err) == "table" and err.code, count())
        local waited = pcall(db.transaction, db, function()
            db:exec("INSERT INTO items (name) VALUES (?)", { "waiting" })
            sys.sleep(0)
        end)
        emit(waited, count())
        emit(db:transaction(function()
            db:exec("INSERT INTO items (name) VALUES (?)", { "saved" })
            return "done", 2
        end))
        emit(count())
        emit((pcall(db.exec, db, "NOT SQL")))
        emit((pcall(db.exec, db, "INSERT INTO items (name) VALUES (?)", { {} })))
        db:close()
        emit((pcall(db.query, db, "SELECT 1")))
        "#,
    )
    .unwrap();
    assert_eq!(
        seen[..3],
        [json!(false), json!(9), json!(0)],
        "the original table came back and nothing was kept"
    );
    assert_eq!(
        seen[3..5],
        [json!(false), json!(0)],
        "waiting inside a transaction fails and rolls back"
    );
    assert_eq!(seen[5..7], [json!("done"), json!(2)]);
    assert_eq!(seen[7], 1);
    assert_eq!(seen[8], false, "bad SQL raises");
    assert_eq!(seen[9], false, "a table cannot be stored");
    assert_eq!(seen[10], false, "a closed database raises");
}

#[test]
fn screen_calls_outside_the_screen_do_nothing_and_bad_ones_raise() {
    let seen = probe(
        r#"
        local sys = require("uji.sys")
        local screen = sys.tty.open()
        emit((pcall(screen.fill, screen, -3, -3, 5, 5)))
        emit(screen:line(-1, 0, "x"))
        emit(screen:line(0, -4, "x"))
        emit(screen:line(10000, 0, "x"))
        emit(screen:text(-1) == nil)
        emit((pcall(screen.cursor, screen, -1, 2)))
        emit((pcall(screen.line, screen, 0, 0, 42)))
        emit((pcall(screen.line, screen, 0, 0, { 42 })))
        emit((pcall(screen.style, screen, { fg = "not-a-colour" })))
        emit((pcall(screen.cursor, screen, 0, 0, "wiggle")))
        emit((pcall(screen.paint, screen, 0, 0, 1, 1, 9999)))
        "#,
    )
    .unwrap();
    assert_eq!(
        seen[..6],
        [
            json!(true),
            json!(0),
            json!(-4),
            json!(0),
            json!(true),
            json!(true)
        ],
        "positions off the screen draw nothing and leave the column where it was"
    );
    assert!(all_false(&seen[6..]), "bad arguments raised: {seen:?}");
}

#[test]
fn a_missing_module_says_where_it_looked() {
    let seen = probe(
        r#"
        local ok, err = pcall(require, "uji-no-such-module")
        emit(ok, err:find("no runtime module", 1, true) ~= nil, err:find("no built-in module", 1, true) ~= nil)
        emit((pcall(require, "uji.sys.no_such_thing")))
        "#,
    )
    .unwrap();
    assert_eq!(seen, [json!(false), json!(true), json!(true), json!(false)]);
}

#[test]
fn text_that_is_not_utf8_is_still_handled() {
    let seen = probe(
        r#"
        local sys = require("uji.sys")
        local bad = "\255\255abc"
        emit(sys.width("\255"))
        emit(sys.lossy(bad))
        emit({ sys.regex("abc"):find(bad) })
        emit(sys.fuzzy("ab", { "\255xab", "zzz" }))
        emit(#sys.markdown("\255 *text*") > 0)
        emit(sys.base64.encode(bad))
        "#,
    )
    .unwrap();
    assert_eq!(seen[0], 1);
    assert_eq!(seen[1], "\u{FFFD}\u{FFFD}abc");
    assert_eq!(
        seen[2],
        json!([3, 5]),
        "positions point into the original bytes"
    );
    assert_eq!(seen[3], json!([1]));
    assert_eq!(seen[4], true);
    assert_eq!(seen[5], "//9hYmM=");
}

#[test]
fn missing_names_are_nil() {
    let seen = probe(
        r#"
        local sys = require("uji.sys")
        emit(uji[nil] == nil, uji[{}] == nil, uji.no_such_name == nil, sys.no_such_module == nil)
        emit(sys.no_such_module == nil)
        "#,
    )
    .unwrap();
    assert_eq!(seen, vec![json!(true); 5]);
}

#[test]
fn a_wait_inside_your_own_coroutine_does_not_finish() {
    let seen = probe(
        r#"
        local sys = require("uji.sys")
        local result = coroutine.wrap(function()
            sys.sleep(0.01)
            return "done"
        end)()
        emit(result ~= "done")
        "#,
    )
    .unwrap();
    assert_eq!(seen[0], true);
}

#[test]
fn nothing_runs_after_exit() {
    let seen = probe(
        r#"
        local sys = require("uji.sys")
        emit("before")
        sys.task.spawn(function()
            emit("other task")
        end)
        sys.os.exit(0)
        sys.sleep(0)
        emit("after")
        "#,
    )
    .unwrap();
    assert_eq!(seen, [json!("before")]);
}
