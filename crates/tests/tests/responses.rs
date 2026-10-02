use std::error::Error;

use serde_json::json;
use uji_tests::{Sandbox, probe};

#[test]
fn responses_encode_history_and_preserve_opaque_items() -> Result<(), Box<dyn Error>> {
    let seen = probe(
        r#"
        local codec = require('uji.wires.openai_responses')
        local sys = require('uji.sys')
        local request = {
            model = 'fixture', system = 'Instructions', tools = {
                {name = 'read_file', description = 'Read', parameters = {type = 'object'}}
            }, effort = 'medium', cache = 'short', max_output = 4096, session = 'conversation',
            provider = {id = 'fixture', base_url = 'https://responses.invalid/v1',
                compat = {session_header = 'x-opencode-session'}}, auth = {key = 'synthetic-key'},
            messages = {{type = 'user', text = 'Read', images = {{media_type = 'image/png', data = 'aW1hZ2U='}}}}
        }
        local answer = assert(codec.decode({status = 'completed', output = uji.json.array({
            {type = 'reasoning', id = 'reasoning-id', encrypted_content = 'opaque', summary = uji.json.array({}),
                extra = uji.json.null},
            {type = 'function_call', id = 'item-id', call_id = 'call-id', name = 'read_file', arguments = '{"path":"a"}'}
        }), usage = {input_tokens = 30, output_tokens = 10, input_tokens_details = {cached_tokens = 20}}}, request))
        request.messages[#request.messages + 1] = {type = 'assistant', text = answer.text,
            tool_calls = answer.tool_calls, wire_state = answer.wire_state}
        request.messages[#request.messages + 1] = {type = 'tool', name = 'read_file',
            tool_call_id = 'call-id', content = 'file content'}
        sys.net.open = function(opts)
            emit({body = sys.json.decode(opts.body), headers = opts.headers, url = opts.url, usage = answer.usage})
            return nil, 'recorded offline'
        end
        codec.stream(request, {text = function() end, reasoning = function() end, done = function() error('network') end,
            fail = function(failure) assert(failure.kind == 'http') end})
        request.auth.key = 'changed-key'
        local ok, err = pcall(codec.encode, request)
        emit({changed = ok, error = tostring(err)})
        request.auth.key = 'synthetic-key'
        request.messages[2].wire_state = nil
        emit(codec.encode(request).input)
    "#,
    )?;
    let sent = &seen[0];
    assert_eq!(sent["url"], "https://responses.invalid/v1/responses");
    assert_eq!(sent["headers"]["Authorization"], "Bearer synthetic-key");
    assert_eq!(sent["headers"]["x-opencode-session"], "conversation");
    assert_eq!(sent["body"]["instructions"], "Instructions");
    assert_eq!(sent["body"]["store"], false);
    assert_eq!(
        sent["body"]["include"],
        json!(["reasoning.encrypted_content"])
    );
    assert_eq!(sent["body"]["reasoning"]["effort"], "medium");
    assert_eq!(sent["body"]["max_output_tokens"], 4096);
    assert_eq!(sent["body"]["tools"][0]["name"], "read_file");
    assert_eq!(sent["body"]["tools"][0]["strict"], false);
    assert_eq!(
        sent["body"]["input"][0]["content"][0]["image_url"],
        "data:image/png;base64,aW1hZ2U="
    );
    assert_eq!(sent["body"]["input"][1]["encrypted_content"], "opaque");
    assert!(
        sent["body"]["input"][1]
            .get("extra")
            .is_some_and(|v| v.is_null())
    );
    assert_eq!(sent["body"]["input"][2]["id"], "item-id");
    assert_eq!(
        sent["body"]["input"][3],
        json!({"type":"function_call_output", "call_id":"call-id", "output":"file content"})
    );
    assert_eq!(
        sent["usage"],
        json!({"input":10,"output":10,"cache_read":20,"cache_write":0})
    );
    assert_eq!(seen[1]["changed"], false);
    assert!(
        seen[1]["error"]
            .as_str()
            .unwrap()
            .contains("different API key")
    );
    assert_eq!(seen[2][1]["call_id"], "call-id");
    Ok(())
}

#[test]
fn responses_do_not_replay_opaque_items_across_provider_or_endpoint_changes()
-> Result<(), Box<dyn Error>> {
    let seen = probe(
        r#"
        local codec = require('uji.wires.openai_responses')
        local request = {model = 'fixture', system = '', tools = {}, effort = 'off', cache = 'off',
            max_output = 64, provider = {id = 'original', base_url = 'https://original.invalid'},
            auth = {key = 'synthetic-key'}, messages = {}}
        local answer = assert(codec.decode({status = 'completed', output = {{type = 'reasoning',
            encrypted_content = 'opaque'}, {type = 'function_call', call_id = 'call', name = 'read_file',
            arguments = '{"path":"notes.txt"}'}}}, request))
        request.messages = {{type = 'assistant', text = 'Read the file',
            tool_calls = answer.tool_calls, wire_state = answer.wire_state}}
        for _, provider in ipairs({{id = 'different', base_url = 'https://original.invalid'},
            {id = 'original', base_url = 'https://different.invalid'}}) do
            request.provider = provider
            emit(codec.encode(request).input)
        end
    "#,
    )?;
    for input in seen {
        assert_eq!(input.as_array().unwrap().len(), 2);
        assert_eq!(input[0]["role"], "assistant");
        assert_eq!(input[0]["content"], "Read the file");
        assert_eq!(input[1]["type"], "function_call");
        assert_eq!(input[1]["call_id"], "call");
        assert_eq!(input[1]["arguments"], "{\"path\":\"notes.txt\"}");
        assert!(!input.to_string().contains("opaque"));
    }
    Ok(())
}

#[test]
fn responses_reject_incomplete_and_invalid_calls() -> Result<(), Box<dyn Error>> {
    let seen = probe(
        r#"
        local codec = require('uji.wires.openai_responses')
        local request = {provider = {id = 'fixture', base_url = 'https://responses.invalid'}, auth = {key = 'key'}}
        for _, status in ipairs({'failed', 'incomplete', 'in_progress'}) do
            local answer, failure = codec.decode({status = status, incomplete_details = {reason = 'max_output_tokens'}}, request)
            assert(not answer and failure.kind == 'provider')
        end
        local base = {status = 'completed', output = {{type = 'function_call', call_id = 'c', name = 'read_file', arguments = '['}}}
        assert(not pcall(codec.decode, base, request))
        base.output[1].arguments = '[]'
        assert(not pcall(codec.decode, base, request))
        base.output[1].arguments = '{}'
        base.output[1].call_id = ''
        assert(not pcall(codec.decode, base, request))
        local answer, failure = codec.decode({status = 'completed', output = {{type = 'message', content = {{type = 'refusal', refusal = 'Denied'}}}}}, request)
        assert(not answer and failure.message == 'Denied')
        emit({rejected = true})
    "#,
    )?;
    assert_eq!(seen[0]["rejected"], true);
    Ok(())
}

#[test]
fn wire_state_survives_live_tool_continuations_and_reload() -> Result<(), Box<dyn Error>> {
    let sandbox = Sandbox::new("responses-state")?;
    sandbox.file("notes.txt", "actual file content")?;
    let seen = sandbox.probe(r#"
        local app, model, sys = require('uji.app'), require('uji.model'), require('uji.sys')
        local state = {wire = 'test-wire', output = '[{"encrypted_content":"opaque","extra":null}]'}
        local calls = 0
        uji.tool.policy({default = 'allow', read_file = {default = 'allow'}})
        uji.wire.add('state-fixture', {stream = function(request, reply)
            calls = calls + 1
            if calls == 1 then
                reply.done({text = '', tool_calls = {{id = 'read-call', name = 'read_file', arguments = '{"path":"notes.txt"}'}}, wire_state = state})
            else
                emit({continuation = uji.json.decode(uji.json.encode(request.messages))})
                reply.done({text = 'done', wire_state = state})
            end
        end})
        uji.provider.add({id = 'state-fixture', name = 'Fixture', wire = 'state-fixture', base_url = 'https://unused.invalid', models = {'m'}})
        app.store:set_setting('llm.provider', 'state-fixture')
        app.store:set_setting('llm.model', 'm')
        model.resolve()
        uji.session.set_title('Avoid auxiliary title call')
        local finished = sys.promise()
        uji.on('turn_finished', function() finished:resolve(true) end)
        uji.session.submit('Read notes.txt')
        finished:await()
        emit({session = app.session.id, messages = app.session:messages(), calls = calls})
    "#)?;
    let messages = &seen[1]["messages"];
    assert_eq!(seen[1]["calls"], 2);
    assert_eq!(messages[2]["tool_call_id"], "read-call");
    assert!(
        messages[2]["content"]
            .as_str()
            .unwrap()
            .contains("actual file content")
    );
    assert_eq!(
        seen[0]["continuation"][1]["wire_state"],
        messages[1]["wire_state"]
    );
    assert_eq!(messages[3]["wire_state"], messages[1]["wire_state"]);
    let session = serde_json::to_string(&seen[1]["session"])?;
    let restored = sandbox.probe(&format!(
        "local session = assert(require('uji.app').store:session({session})); emit(require('uji.agent.view').build(session:entries()))"
    ))?;
    assert_eq!(restored[0][1]["wire_state"], messages[1]["wire_state"]);
    assert_eq!(restored[0][3]["wire_state"], messages[3]["wire_state"]);
    Ok(())
}
