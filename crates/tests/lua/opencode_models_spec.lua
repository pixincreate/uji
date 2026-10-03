it("fetches model IDs and metadata through the existing HTTP API", function()
    local sandbox = require("support.sandbox")
    local listed = require("uji.builtin.opencode")
    local document = sandbox.read(sandbox.fixtures .. "/opencode_metadata.json")
    local calls = {}
    uji.http.request = function(opts)
        assert.equal(5, opts.timeout)
        assert.is_nil(opts.headers.Authorization)
        calls[#calls + 1] = opts.url
        if opts.url == "https://models.dev/api.json" then
            return { status = 200, body = document }
        end
        return { status = 200, body = '{"data":[{"id":"space-bunny-free"},{"id":"unsupported"},{"id":"space-bunny-free"}]}' }
    end
    local models = listed("https://opencode.ai/zen/go/v1", "opencode-go", { ["space-bunny-free"] = true })
    assert.same({ "https://opencode.ai/zen/go/v1/models", "https://models.dev/api.json" }, calls)
    assert.equal(1, #models)
    assert.equal("space-bunny-free", models[1].id)
    assert.equal(1048576, models[1].context)
    assert.equal(524288, models[1].output)
    assert.is_true(models[1].reasoning and models[1].images and models[1].cache)
    assert.same({ "low", "medium", "high", "xhigh", "max" }, models[1].efforts)
end)

it("returns no bundled models when an inventory or metadata request fails", function()
    local listed = require("uji.builtin.opencode")
    local notices = {}
    uji.notify = function(text)
        notices[#notices + 1] = text
    end
    for _, response in ipairs({
        { status = 503, body = "unavailable" },
        { status = 200, body = "invalid JSON" },
        { status = 200, body = '{"data":null}' },
    }) do
        uji.http.request = function()
            return response
        end
        assert.same({}, listed("https://opencode.ai/zen/go/v1", "opencode-go", { ["space-bunny-free"] = true }))
    end
    assert.equal(0, #notices)
end)

it("keeps invalid remote efforts from escaping provider registration", function()
    local listed = require("uji.builtin.opencode")
    for _, values in ipairs({ "not a list", { "none" } }) do
        local metadata = {
            fixture = {
                models = {
                    model = {
                        limit = { context = 100, output = 10 },
                        reasoning = true,
                        reasoning_options = { { type = "effort", values = values } },
                    },
                },
            },
        }
        uji.http.request = function(opts)
            return {
                status = 200,
                body = opts.url == "https://models.dev/api.json" and uji.json.encode(metadata) or '{"data":[{"id":"model"}]}',
            }
        end
        local models = listed("https://fixture.invalid", "fixture", { model = true })
        assert.same({}, models)
        uji.provider.add({
            id = "metadata-fixture",
            name = "Fixture",
            base_url = "https://fixture.invalid",
            api = uji.api.openai(),
            models = models,
        })
    end
end)

it("routes Zen requests independently of Go through the existing APIs", function()
    local sandbox = require("support.sandbox")
    local document = sandbox.read(sandbox.fixtures .. "/opencode_metadata.json")
    uji.http.request = function(opts)
        return {
            status = 200,
            body = opts.url == "https://models.dev/api.json" and document
                or '{"data":[{"id":"big-pickle"},{"id":"claude-sonnet-4-6"},{"id":"muse-spark-1.3-contributor-free"},{"id":"gemini-3.8-flash"}]}',
        }
    end
    package.loaded["uji.builtin.providers.opencode-zen"] = nil
    require("uji.builtin.providers.opencode-zen")
    local provider = require("uji.core.catalog").get("opencode-zen")
    local routes = {
        ["big-pickle"] = "/chat/completions",
        ["claude-sonnet-4-6"] = "/messages",
        ["muse-spark-1.3-contributor-free"] = "/responses",
        ["gemini-3.8-flash"] = "/models/gemini-3.8-flash:streamGenerateContent?alt=sse",
    }
    local seen = {}
    require("uji.sys").net.open = function(opts)
        seen[#seen + 1] = opts
        return nil, "recorded; no network"
    end
    for id, suffix in pairs(routes) do
        provider.api:stream({
            model = id,
            provider = { id = provider.id, base_url = provider.base_url },
            auth = { key = "synthetic-key" },
            session = "zen-session",
            messages = { { type = "user", text = "Hello" } },
            tools = {},
            reasoning = false,
            max_output = 16,
            cache = "off",
        }, { fail = function() end })
        local request = seen[#seen]
        assert.equal(provider.base_url .. suffix, request.url)
        assert.equal("zen-session", request.headers["x-opencode-session"])
    end
    assert.equal(4, #seen)
end)

it("keeps rejection diagnostics bounded and removes echoed API keys", function()
    require("uji.sys").net.open = function()
        return {
            status = 403,
            headers = {},
            read = function()
                return "Region denied: Bearer fixt\27ure-secret " .. string.rep("x", 3000)
            end,
        }
    end
    local _, failure = uji.api.openai():stream({
        model = "fixture",
        provider = { base_url = "https://fixture.invalid" },
        auth = { key = "fixture-secret" },
        messages = {},
        tools = {},
        max_output = 16,
        cache = "off",
    }, { fail = function() end })
    assert.equal(403, failure.status)
    assert.is_truthy(failure.message:find("Region denied", 1, true))
    assert.is_nil(failure.message:find("fixture-secret", 1, true))
    assert.is_nil(failure.message:find("\27", 1, true))
    assert.is_true(#failure.message <= 2100)
end)
