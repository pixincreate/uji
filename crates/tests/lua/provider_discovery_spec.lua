it("replaces discovered models and fails closed without fetching unrelated providers", function()
    local sys = require("uji.sys")
    local model = require("uji.core.model")
    local requests, streams = 0, 0
    local rows = { { id = "remote-a", context = 1000, output = 100, reasoning = true, images = false } }
    uji.provider.add({
        id = "discovery-test",
        name = "Discovery test",
        base_url = "https://discovery.invalid/v1",
        auth_env = {},
        api = {
            stream = function()
                streams = streams + 1
            end,
        },
        discover = function()
            requests = requests + 1
            return rows, rows == nil and { kind = "provider", message = "catalog unavailable" } or nil
        end,
    })
    uji.provider.add({
        id = "unselected",
        name = "Unselected",
        base_url = "https://unused.invalid",
        api = {
            stream = function()
                error("unselected")
            end,
        },
        discover = function()
            error("unselected discovery")
        end,
    })
    assert.equal(0, requests)
    uji.provider.list()
    assert.equal(0, requests)
    local found = assert(uji.provider.refresh("discovery-test"))
    assert.equal("remote-a", found.models[1].id)
    assert.is_false(found.models[1].images)
    rows = { { id = "remote-b", context = 2000, output = 200 } }
    found = assert(uji.provider.refresh("discovery-test"))
    assert.equal(1, #found.models)
    assert.equal("remote-b", found.models[1].id)
    rows = nil
    local empty, failure = uji.provider.refresh("discovery-test")
    assert.is_nil(empty)
    assert.equal("catalog unavailable", failure.message)
    model.resolve({ provider = "discovery-test", model = "remote-a" })
    local answer, denied = model.stream({ messages = {}, tools = {} })
    assert.is_nil(answer)
    assert.equal("catalog unavailable", denied.message)
    assert.equal(0, streams)
    assert.equal(3, requests)
    rows = { { id = "remote-b", context = 2000, output = 200 } }
    assert(uji.provider.refresh("discovery-test"))
    local _, unavailable = model.stream({ model = "remote-a", messages = {}, tools = {} })
    assert.is_truthy(unavailable.message:find("unavailable", 1, true))
    assert.equal(0, streams)
    sys.sleep(0)
end)

it("enriches public inventories from recorded metadata and clears them on refresh failure", function()
    local sys = require("uji.sys")
    local catalog = require("uji.core.catalog")
    local sandbox = require("support.sandbox")
    -- Selected fields captured from models.dev/api.json on 2026-10-03.
    local document = sandbox.read(sandbox.fixtures .. "/opencode_metadata.json")
    local requests, broken = {}, false
    sys.net.request = function(opts)
        assert.is_nil(opts.headers.Authorization)
        assert.is_nil(opts.headers["x-api-key"])
        assert.equal(15, opts.timeout)
        requests[#requests + 1] = opts.url
        if broken then
            return nil, "discovery offline"
        end
        if opts.url == "https://models.dev/api.json" then
            return { status = 200, body = document }
        end
        local ids = opts.url:find("/go/", 1, true)
                and { "space-bunny-free", "minimax-m3", "muse-spark-1.3-contributor", "kimi-k2-thinking" }
            or { "big-pickle", "claude-sonnet-4-6", "muse-spark-1.3-contributor-free", "gemini-3.8-flash" }
        local data = sys.json.array({})
        for _, id in ipairs(ids) do
            data[#data + 1] = { id = id, object = "model", owned_by = "opencode" }
        end
        return { status = 200, body = sys.json.encode({ object = "list", data = data }) }
    end
    local go = assert(uji.provider.refresh("opencode-go"))
    assert.equal(3, #go.models)
    assert.equal(2, #requests)
    assert.equal("https://opencode.ai/zen/go/v1/models", requests[1])
    local space = catalog.get("opencode-go"):model("space-bunny-free")
    assert.equal(1048576, space.context)
    assert.equal(524288, space.output)
    assert.same({ "low", "medium", "high", "xhigh", "max" }, space.efforts)
    assert.is_true(space.images and space.reasoning and space.cache)
    assert.is_nil(catalog.get("opencode-go"):model("kimi-k2-thinking"))
    local zen = assert(uji.provider.refresh("opencode-zen"))
    assert.equal(4, #zen.models)
    assert.is_false(catalog.get("opencode-zen"):model("big-pickle").images)
    assert.equal(200000, catalog.get("opencode-zen"):model("big-pickle").context)
    broken = true
    local ready, failure = uji.provider.refresh("opencode-go")
    assert.is_nil(ready)
    assert.is_truthy(failure.message:find("discovery offline", 1, true))
    assert.equal(0, #catalog.get("opencode-go").models)
    local before, blocked = #requests, nil
    catalog.get("opencode-go").api:stream({ model = "space-bunny-free" }, {
        fail = function(err)
            blocked = err
        end,
    })
    assert.equal(before, #requests)
    assert.is_truthy(blocked.message:find("discovery offline", 1, true))
end)

it("reports bounded rejection details without echoing credentials", function()
    local sys = require("uji.sys")
    local api = uji.api.openai()
    sys.net.open = function()
        return {
            status = 403,
            headers = {},
            read = function()
                return "Region is not permitted. Bearer fixt\27ure-secret\27[31m " .. string.rep("x", 3000)
            end,
        }
    end
    local _, failure = api:stream({
        model = "big-pickle",
        provider = { base_url = "https://zen.invalid" },
        auth = { key = "fixture-secret" },
        messages = { { type = "user", text = "Hello" } },
        tools = {},
        max_output = 16,
        cache = "off",
    }, { text = function() end, reasoning = function() end, done = function() end, fail = function() end })
    assert.equal("auth", failure.kind)
    assert.equal(403, failure.status)
    assert.is_truthy(failure.message:find("Region is not permitted", 1, true))
    assert.is_nil(failure.message:find("fixture-secret", 1, true))
    assert.is_nil(failure.message:find("\27", 1, true))
    assert.is_true(#failure.message <= 2100)
end)

it("discovers a default on first use without overwriting a later selection", function()
    local model = require("uji.core.model")
    local sent = {}
    uji.provider.add({
        id = "lazy-test",
        name = "Lazy test",
        base_url = "https://lazy.invalid",
        auth_env = {},
        api = {
            stream = function(_, request, reply)
                sent[#sent + 1] = request.model
                reply.done({ text = "fixture answer", tool_calls = {}, usage = { input = 0, output = 0, cache_read = 0, cache_write = 0 } })
            end,
        },
        discover = function()
            return { { id = "a", context = 1000, output = 100 }, { id = "b", context = 2000, output = 200 } }
        end,
    })
    uji.auth.save_key("lazy-test", "synthetic-key")
    model.resolve({ provider = "lazy-test", model = "" })
    assert(model.stream({ model = "", messages = {}, tools = {} }))
    assert.equal("a", sent[1])
    assert.equal("a", uji.model.current().model)
    model.resolve({ provider = "lazy-test", model = "" })
    assert(model.generate({ system = "Title", messages = {} }))
    assert.equal("a", sent[2])
    model.resolve({ provider = "lazy-test", model = "b" })
    assert(model.stream({ model = "a", messages = {}, tools = {} }))
    assert.equal("a", sent[3])
    assert.equal("b", uji.model.current().model)
    model.set_setting("llm.provider", "lazy-test")
    model.set_setting("llm.base_url", "https://different.invalid")
    model.resolve({ model = "a" })
    local answer, failure = model.stream({ model = "a", messages = {}, tools = {} })
    assert.is_nil(answer)
    assert.is_truthy(failure.message:find("base URL override", 1, true))
    assert.equal(3, #sent)
end)

it("coalesces discovery and applies overrides only to remote members", function()
    local sys = require("uji.sys")
    local wait, calls = sys.promise(), 0
    uji.provider.add({
        id = "coalesced",
        name = "Coalesced",
        base_url = "https://coalesced.invalid",
        api = { stream = function() end },
        discover = function()
            calls = calls + 1
            wait:await()
            return { { id = "a", context = 100, output = 10 } }
        end,
    })
    uji.provider.add({ id = "coalesced", models = { { id = "a", context = 200, output = 20 }, { id = "local-only" } } })
    local first, second = sys.promise(), sys.promise()
    uji.task.spawn(function()
        first:resolve(uji.provider.refresh("coalesced"))
    end)
    uji.task.spawn(function()
        second:resolve(uji.provider.refresh("coalesced"))
    end)
    sys.sleep(0)
    wait:resolve(true)
    local a, b = first:await(), second:await()
    assert.equal(1, calls)
    assert.equal(1, #a.models)
    assert.same(a.models, b.models)
    assert.equal(200, a.models[1].context)
    uji.provider.add({
        id = "static",
        name = "Static",
        base_url = "https://static.invalid",
        api = { stream = function() end },
        models = { "one" },
    })
    uji.provider.add({ id = "static", models = { "two" } })
    assert.equal(2, #uji.provider.refresh("static").models)
end)

it("settles shared discovery when its initiating caller is cancelled", function()
    local sys = require("uji.sys")
    local catalog = require("uji.core.catalog")
    local entered, release = sys.promise(), sys.promise()
    local calls = 0
    uji.provider.add({
        id = "cancel-discovery",
        name = "Cancellation fixture",
        base_url = "https://cancel.invalid",
        api = { stream = function() end },
        discover = function()
            calls = calls + 1
            entered:resolve(true)
            release:await()
            return { { id = "available", context = 100, output = 10 } }
        end,
    })
    local caller = uji.task.spawn(function()
        uji.provider.refresh("cancel-discovery")
        error("cancelled caller resumed")
    end)
    entered:await()
    caller:cancel()
    release:resolve(true)
    sys.sleep(0)
    assert.equal("ready", catalog.get("cancel-discovery").discovery)
    local found = assert(uji.provider.refresh("cancel-discovery"))
    assert.equal("available", found.models[1].id)
    assert.equal(2, calls)
end)

it("routes Zen models through their service-specific formats", function()
    local catalog = require("uji.core.catalog")
    local sys = require("uji.sys")
    local expected = {
        ["big-pickle"] = "/chat/completions",
        ["minimax-m3"] = "/chat/completions",
        ["qwen3.8-max"] = "/chat/completions",
        ["claude-sonnet-4-6"] = "/messages",
        ["muse-spark-1.3-contributor-free"] = "/responses",
        ["gemini-3.8-flash"] = "/models/gemini-3.8-flash:streamGenerateContent?alt=sse",
    }
    uji.provider.add({
        id = "opencode-zen",
        discover = function()
            local rows = {}
            for id in pairs(expected) do
                rows[#rows + 1] = { id = id, context = 1000000, output = 4096 }
            end
            return rows
        end,
    })
    assert(uji.provider.refresh("opencode-zen"))
    local sent = {}
    sys.net.open = function(opts)
        assert.equal("uji", opts.headers["User-Agent"])
        assert.equal("zen-session", opts.headers["x-opencode-session"])
        sent[#sent + 1] = opts.url
        return nil, "recorded; no network"
    end
    local zen = catalog.get("opencode-zen")
    for id, path in pairs(expected) do
        zen.api:stream({
            model = id,
            provider = { id = zen.id, base_url = zen.base_url },
            auth = { key = "fixture-key" },
            session = "zen-session",
            messages = { { type = "user", text = "Hello" } },
            tools = {},
            reasoning = false,
            max_output = 4096,
            cache = "off",
        }, { text = function() end, reasoning = function() end, done = function() end, fail = function() end })
        assert.equal(zen.base_url .. path, sent[#sent])
    end
    assert.equal(6, #sent)
end)
