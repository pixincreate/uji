use std::error::Error;

use serde_json::json;
use uji_tests::probe;

#[test]
fn go_routes_each_model_without_changing_other_providers() -> Result<(), Box<dyn Error>> {
    let seen = probe(
        r#"
        local catalog = require('uji.core.catalog')
        local sys = require('uji.sys')
        local go = assert(catalog.get('opencode-go'))
        local paths = {}
        for _, id in ipairs({'minimax-m3','minimax-m2.7','qwen3.8-max','qwen3.8-flash','qwen3.7-plus'}) do
            paths[id] = '/messages'
        end
        for _, id in ipairs({'grok-4.7','grok-4.6','gpt-6-luna','gpt-5.6-luna',
            'muse-spark-1.3-contributor','muse-spark-1.2-contributor'}) do
            paths[id] = '/responses'
        end
        local requests = {}
        sys.net.open = function(opts)
            requests[#requests+1] = {url=opts.url,method=opts.method,headers=opts.headers,
                body=uji.json.decode(opts.body)}
            return nil, 'recorded; no network'
        end
        local reply = {text=function() end,reasoning=function() end,
            done=function() error('unexpected provider response') end,fail=function() end}
        for _, model in ipairs(go.models) do
            go.api:stream({model=model.id,provider={id=go.id,base_url=go.base_url},auth={key='fixture-key'},
                session='parent',system='Follow instructions.',messages={{type='user',text='Read notes.'},
                    {type='assistant',text='',tool_calls={{id='call_1',name='read_file',arguments='{"path":"notes.txt"}'}}},
                    {type='tool',name='read_file',tool_call_id='call_1',content='alpha'}},
                tools={{name='read_file',description='Read a file.',parameters={type='object',
                    properties={path={type='string'}}}}},reasoning=false,effort='off',max_output=4096,cache='off'},reply)
            local sent = requests[#requests]
            assert(sent.url == go.base_url .. (paths[model.id] or '/chat/completions'))
            assert(sent.body.model == model.id and sent.method == 'POST')
            assert(sent.headers['User-Agent']=='uji' and sent.headers['x-opencode-session']=='parent')
            if paths[model.id]=='/messages' then
                assert(sent.headers['x-api-key']=='fixture-key' and sent.headers['anthropic-version']=='2023-06-01')
            else assert(sent.headers.Authorization=='Bearer fixture-key') end
        end
        local before = #requests
        local failed
        go.api:stream({model='unsupported'}, {fail=function(err) failed=err end})
        assert(#requests==before and failed.kind=='provider')
        local zen = assert(catalog.get('opencode-zen'))
        assert(zen.base_url=='https://opencode.ai/zen/v1')
        assert(zen.api:url({provider={base_url=zen.base_url}})==zen.base_url..'/chat/completions')
        local openai = assert(catalog.get('openai'))
        assert(openai.api:url({provider={base_url=openai.base_url}})=='https://api.openai.com/v1/responses')
        emit(requests)
    "#,
    )?;
    let requests = seen[0].as_array().ok_or("missing recorded requests")?;
    assert_eq!(requests.len(), 30);
    let sent = |model: &str| {
        requests
            .iter()
            .find(|request| request.pointer("/body/model") == Some(&json!(model)))
            .expect("model request")
    };
    let chat = sent("glm-5.2");
    assert_eq!(
        chat.pointer("/body/messages/0/content"),
        Some(&json!("Follow instructions."))
    );
    assert_eq!(
        chat.pointer("/body/messages/2/tool_calls/0/id"),
        Some(&json!("call_1"))
    );
    assert_eq!(
        chat.pointer("/body/messages/3/tool_call_id"),
        Some(&json!("call_1"))
    );
    let messages = sent("minimax-m3");
    assert_eq!(
        messages.pointer("/body/messages/1/content/0/id"),
        Some(&json!("call_1"))
    );
    assert_eq!(
        messages.pointer("/body/messages/2/content/0/tool_use_id"),
        Some(&json!("call_1"))
    );
    let responses = sent("muse-spark-1.3-contributor");
    assert_eq!(
        responses.pointer("/body/input/0/role"),
        Some(&json!("system"))
    );
    assert_eq!(
        responses.pointer("/body/input/0/content"),
        Some(&json!("Follow instructions."))
    );
    assert_eq!(
        responses.pointer("/body/input/2/call_id"),
        Some(&json!("call_1"))
    );
    assert_eq!(
        responses.pointer("/body/input/3/call_id"),
        Some(&json!("call_1"))
    );
    assert_eq!(responses.pointer("/body/store"), Some(&json!(false)));
    Ok(())
}

#[test]
fn go_headers_follow_the_request_session() -> Result<(), Box<dyn Error>> {
    let seen = probe(
        r#"
        local go = assert(require('uji.core.catalog').get('opencode-go'))
        local sys, headers = require('uji.sys'), {}
        sys.net.open = function(opts)
            headers[#headers+1]=opts.headers
            return nil, 'recorded; no network'
        end
        local reply={text=function() end,reasoning=function() end,done=function() end,fail=function() end}
        for _, model in ipairs({'glm-5.2','minimax-m3','muse-spark-1.3-contributor'}) do
            for _, session in ipairs({'parent','child',''}) do
                go.api:stream({model=model,provider={id=go.id,base_url=go.base_url},auth={key='fixture-key'},
                    session=session,messages={{type='user',text='Hello'}},tools={},reasoning=false,
                    effort='off',max_output=4096,cache='off'},reply)
            end
        end
        emit(headers)
    "#,
    )?;
    let headers = seen[0].as_array().ok_or("missing headers")?;
    assert_eq!(headers.len(), 9);
    for group in headers.chunks_exact(3) {
        assert_eq!(group[0].get("x-opencode-session"), Some(&json!("parent")));
        assert_eq!(group[1].get("x-opencode-session"), Some(&json!("child")));
        assert!(group[2].get("x-opencode-session").is_none());
        assert_eq!(group[2].get("User-Agent"), Some(&json!("uji")));
    }
    Ok(())
}
