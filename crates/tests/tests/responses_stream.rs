use std::error::Error;

use uji_tests::probe;

#[test]
fn responses_stream_progress_and_terminal_failures() -> Result<(), Box<dyn Error>> {
    let seen = probe(
        r#"
        local sys, codec = require('uji.sys'), require('uji.wires.openai_responses')
        local request = {model = 'fixture', system = '', messages = {{type = 'user', text = 'Read'}}, tools = {},
            effort = 'off', cache = 'off', max_output = 4096,
            provider = {id = 'fixture', base_url = 'https://unused.invalid'}, auth = {key = 'synthetic-key'}}
        local output = uji.json.array({{type = 'function_call', id = 'item', call_id = 'call',
            name = 'read_file', arguments = '{"path":"notes.txt"}'}})
        local completed = {type = 'response.completed', response = {status = 'completed', output = output}}
        local clock = 0
        sys.os.clock = function() return clock end
        local function run(events, advance)
            local at, texts, done, failed = 0, {}, 0, 0
            sys.net.open = function()
                return {status = 200, headers = {}, line = function(_, timeout)
                    if at >= #events then return nil end
                    if advance > timeout then return false end
                    clock, at = clock + advance, at + 1
                    local event = events[at]
                    return 'data: ' .. (type(event) == 'string' and event or uji.json.encode(event))
                end}
            end
            local answer, failure = codec.stream(request, {
                text = function(delta) texts[#texts + 1] = delta end, reasoning = function() end,
                done = function() done = done + 1 end, fail = function() failed = failed + 1 end,
            })
            return {answer = answer, failure = failure, done = done, failed = failed, text = table.concat(texts),
                call_count = answer and #answer.tool_calls}
        end
        -- Each argument delta arrives before the 120s idle timeout; the total exceeds it.
        emit(run({{type = 'response.output_item.added', output_index = 0, item = output[1]},
            {type = 'response.function_call_arguments.delta', output_index = 0, delta = '{'},
            {type = 'response.function_call_arguments.delta', output_index = 0, delta = '"path"'},
            {type = 'response.output_item.done', output_index = 0, item = output[1]}, completed}, 50))
        emit(run({'not JSON'}, 0))
        emit(run({{type = 'response.output_text.delta', delta = 'partial'}}, 0))
        emit(run({{type = 'response.failed', response = {status = 'failed', error = {message = 'fixture failure'}}}}, 0))
        emit(run({{type = 'response.incomplete', response = {status = 'incomplete', error = uji.json.null, incomplete_details = {reason = 'max_output_tokens'}}}}, 0))
        emit(run({{type = 'error', message = 'fixture stream error'}}, 0))
        emit(run({{type = 'response.completed', response = {status = 'completed', output = {{type = 'function_call', call_id = 'call', name = 'read_file', arguments = '['}}}}}, 0))
        emit(run({{type = 'response.output_text.delta', delta = 'display'},
            {type = 'response.completed', response = {status = 'completed', output = {{type = 'message', content = {{type = 'output_text', text = 'display'}}}}}}}, 0))
    "#,
    )?;
    assert_eq!(seen[0]["done"], 1);
    assert_eq!(seen[0]["failed"], 0);
    assert_eq!(seen[0]["answer"]["tool_calls"][0]["id"], "call");
    assert_eq!(
        seen[0]["answer"]["tool_calls"][0]["arguments"],
        r#"{"path":"notes.txt"}"#
    );
    for outcome in &seen[1..7] {
        assert_eq!(outcome["done"], 0);
        assert_eq!(outcome["failed"], 1);
        assert!(outcome.get("answer").is_none());
    }
    assert_eq!(seen[2]["failure"]["kind"], "http");
    assert_eq!(seen[3]["failure"]["message"], "fixture failure");
    assert_eq!(seen[4]["failure"]["message"], "max_output_tokens");
    assert_eq!(seen[7]["answer"]["text"], "display");
    assert_eq!(seen[7]["text"], "display");
    assert_eq!(seen[7]["call_count"], 0);
    Ok(())
}
