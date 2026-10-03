local BASE_URL = "https://opencode.ai/zen/go/v1"
local listed = require("uji.builtin.opencode")
local formats = {}

local function group(api, ids)
    local headers = api.headers
    function api:headers(request, ...)
        local out = headers(self, request, ...)
        out["User-Agent"] = "uji"
        if request.session and request.session ~= "" then
            out["x-opencode-session"] = request.session
        end
        return out
    end
    for id in ids:gmatch("%S+") do
        formats[id] = api
    end
end

-- Endpoint formats are service-specific: opencode.ai/docs/go/#endpoints.
group(
    uji.api.openai(),
    [[glm-5.2 glm-5.3 glm-5.3-flash kimi-k2.6 kimi-k2.7-code kimi-k3
    longcat-2.0 longcat-2.5-preview-free deepseek-v4-pro deepseek-v4-flash
    deepseek-v4.1-flash deepseek-v4-flash-vision-exp mimo-v2.5 mimo-v2.5-pro
    mimo-v2.6-flash mimo-v2.6-pro hy4-preview hy3 space-bunny-free]]
)
group(uji.api.anthropic(), [[minimax-m3 minimax-m2.7 qwen3.8-max qwen3.8-flash qwen3.7-plus]])
group(
    uji.api.responses(),
    [[grok-4.7 grok-4.6 gpt-6-luna gpt-5.6-luna
    muse-spark-1.3-contributor muse-spark-1.2-contributor]]
)

local Go = uji.class()

function Go:efforts(model)
    local api = formats[model]
    return api and api.efforts and api:efforts(model) or { "off", "minimal", "low", "medium", "high" }
end

function Go:stream(request, reply)
    local api = formats[request.model]
    local provider = require("uji.core.catalog").get("opencode-go")
    if not api or not provider:model(request.model) then
        return reply.fail({ kind = "provider", message = "unsupported or unavailable OpenCode Go model: " .. tostring(request.model) })
    end
    return api:stream(request, reply)
end

uji.provider.add({
    id = "opencode-go",
    name = "OpenCode Go",
    api = Go(),
    base_url = BASE_URL,
    auth_env = { "OPENCODE_API_KEY" },
    models = listed(BASE_URL, "opencode-go", formats),
})
