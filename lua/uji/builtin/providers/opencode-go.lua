local formats, models = {}, {}

local function group(api, specs)
    local headers = api.headers
    function api:headers(request, ...)
        local out = headers(self, request, ...)
        out["User-Agent"] = "uji"
        if request.session and request.session ~= "" then
            out["x-opencode-session"] = request.session
        end
        return out
    end
    for _, model in ipairs(specs) do
        formats[model.id] = api
        models[#models + 1] = model
    end
end

-- Endpoint formats: opencode.ai/docs/go/#endpoints. Token limits: models.dev.
group(uji.api.openai(), {
    { id = "glm-5.2", context = 1000000, output = 131072, reasoning = true },
    { id = "glm-5.3", context = 1000000, output = 131072, reasoning = true },
    { id = "glm-5.3-flash", context = 1000000, output = 131072, reasoning = true, images = true },
    { id = "kimi-k2.6", context = 262144, output = 65536, reasoning = true, images = true },
    { id = "kimi-k2.7-code", context = 262144, output = 262144, reasoning = true, images = true },
    { id = "kimi-k3", context = 1048576, output = 131072, reasoning = true, images = true },
    { id = "longcat-2.0", context = 1000000, output = 131072, reasoning = true },
    { id = "longcat-2.5-preview-free", context = 1000000, output = 131072, reasoning = true, images = true },
    { id = "deepseek-v4-pro", context = 1000000, output = 384000, reasoning = true },
    { id = "deepseek-v4-flash", context = 1000000, output = 384000, reasoning = true },
    { id = "deepseek-v4.1-flash", context = 1000000, output = 384000, reasoning = true, images = true },
    { id = "deepseek-v4-flash-vision-exp", context = 1000000, output = 384000, reasoning = true, images = true },
    { id = "mimo-v2.5", context = 1000000, output = 128000, reasoning = true, images = true },
    { id = "mimo-v2.5-pro", context = 1048576, output = 128000, reasoning = true },
    { id = "mimo-v2.6-flash", context = 1048576, output = 131072, reasoning = true, images = true },
    { id = "mimo-v2.6-pro", context = 1048576, output = 131072, reasoning = true, images = true },
    { id = "hy4-preview", context = 1024000, output = 64000, reasoning = true },
    { id = "hy3", context = 256000, output = 128000, reasoning = true },
    { id = "space-bunny-free", context = 1048576, output = 524288, reasoning = true, images = true },
})
group(uji.api.anthropic(), {
    { id = "minimax-m3", context = 1000000, output = 131072, reasoning = true, images = true, cache = true },
    { id = "minimax-m2.7", context = 204800, output = 131072, reasoning = true, cache = true },
    { id = "qwen3.8-max", context = 1000000, output = 131072, reasoning = true, images = true, cache = true },
    { id = "qwen3.8-flash", context = 1000000, output = 131072, reasoning = true, images = true, cache = true },
    { id = "qwen3.7-plus", context = 1000000, output = 65536, reasoning = true, images = true, cache = true },
})
group(uji.api.responses(), {
    { id = "grok-4.7", context = 500000, output = 500000, reasoning = true, images = true },
    { id = "grok-4.6", context = 500000, output = 500000, reasoning = true, images = true },
    { id = "gpt-6-luna", context = 1050000, output = 128000, reasoning = true, images = true },
    { id = "gpt-5.6-luna", context = 1050000, output = 128000, reasoning = true, images = true },
    { id = "muse-spark-1.3-contributor", context = 1048576, output = 131072, reasoning = true, images = true },
    { id = "muse-spark-1.2-contributor", context = 1048576, output = 131072, reasoning = true, images = true },
})

local Go = uji.class()

function Go:efforts(model)
    local api = formats[model]
    if api and api.efforts then
        return api:efforts(model)
    end
    return { "off", "minimal", "low", "medium", "high" }
end

function Go:stream(request, reply)
    local api = formats[request.model]
    if not api then
        return reply.fail({ kind = "provider", message = "unsupported OpenCode Go model: " .. request.model })
    end
    return api:stream(request, reply)
end

uji.provider.add({
    id = "opencode-go",
    name = "OpenCode Go",
    api = Go(),
    base_url = "https://opencode.ai/zen/go/v1",
    auth_env = { "OPENCODE_API_KEY" },
    models = models,
})
