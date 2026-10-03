local catalog = require("uji.core.catalog")
local sys = require("uji.sys")

local M = {}
local USER_AGENT = "uji"
local METADATA_URL = "https://models.dev/api.json"

local function fetch(url)
    local response, err = sys.net.request({ url = url, timeout = 15, headers = { ["User-Agent"] = USER_AGENT } })
    if not response then
        error("model discovery request failed: " .. tostring(err), 0)
    end
    if response.status < 200 or response.status >= 300 then
        error("model discovery HTTP " .. response.status .. " at " .. url, 0)
    end
    local decoded = sys.json.decode(response.body)
    if type(decoded) ~= "table" then
        error("invalid model discovery document at " .. url, 0)
    end
    return decoded
end

local function contains(list, wanted)
    for _, value in ipairs(list or {}) do
        if value == wanted then
            return true
        end
    end
    return false
end

local function metadata(id, entry)
    local limits = entry.limit
    if
        type(limits) ~= "table"
        or type(limits.context) ~= "number"
        or limits.context <= 0
        or type(limits.output) ~= "number"
        or limits.output <= 0
    then
        return nil
    end
    local efforts
    for _, option in ipairs(entry.reasoning_options or {}) do
        if option.type == "effort" then
            efforts = {}
            for _, value in ipairs(option.values or {}) do
                if contains(catalog.EFFORTS, value) then
                    efforts[#efforts + 1] = value
                end
            end
            if #efforts == 0 then
                efforts = nil
            end
        end
    end
    return {
        id = id,
        context = limits.context,
        output = limits.output,
        reasoning = entry.reasoning == true,
        efforts = efforts,
        images = contains(entry.modalities and entry.modalities.input, "image"),
        cache = type(entry.cost) == "table" and entry.cost.cache_read ~= nil,
    }
end

-- These are format assignments, not a fallback model catalog. Availability and
-- capabilities come only from successful remote discovery. Unknown formats stay excluded.
local function routes(groups)
    local out = {}
    for format, ids in pairs(groups) do
        for id in ids:gmatch("%S+") do
            out[id] = format
        end
    end
    return out
end

M.go = routes({
    chat = [[glm-5.2 glm-5.3 glm-5.3-flash kimi-k2.6 kimi-k2.7-code kimi-k3
        longcat-2.0 longcat-2.5-preview-free deepseek-v4-pro deepseek-v4-flash
        deepseek-v4.1-flash deepseek-v4-flash-vision-exp mimo-v2.5 mimo-v2.5-pro
        mimo-v2.6-flash mimo-v2.6-pro hy4-preview hy3 space-bunny-free]],
    messages = [[minimax-m3 minimax-m2.7 qwen3.8-max qwen3.8-flash qwen3.7-plus]],
    responses = [[grok-4.7 grok-4.6 gpt-6-luna gpt-5.6-luna
        muse-spark-1.3-contributor muse-spark-1.2-contributor]],
})
M.zen = routes({
    chat = [[qwen3.8-max deepseek-v4.1-flash deepseek-v4-pro deepseek-v4-flash deepseek-v4-flash-vision-exp
        minimax-m3 minimax-m2.7 minimax-m2.5 glm-5.3-flash glm-5.3 glm-5.2 glm-5.1 glm-5
        kimi-k2.5 kimi-k2.6 kimi-k2.7-code kimi-k3 big-pickle space-bunny-free longcat-2.5-preview-free
        fledge-alpha-free mimo-v2.6-flash-free mimo-v2.5-free ling-3.1-flash-free ling-3.0-flash-fin-free
        nemotron-3-ultra-free nemotron-3.5-lightning-free]],
    messages = [[claude-fable-5-1 claude-fable-5 claude-opus-5-5 claude-opus-5 claude-opus-4-8
        claude-opus-4-7 claude-opus-4-6 claude-opus-4-5 claude-sonnet-5 claude-sonnet-4-6
        claude-sonnet-4-5 claude-haiku-4-5 qwen3.8-flash qwen3.7-max qwen3.7-plus qwen3.6-plus qwen3.5-plus]],
    responses = [[gpt-6-astra gpt-6-sol gpt-6.1-sol gpt-6-luna gpt-5.6-sol gpt-5.6-terra gpt-5.6-luna
        gpt-5.5 gpt-5.5-pro gpt-5.4 gpt-5.4-pro gpt-5.4-mini gpt-5.4-nano gpt-5.3-codex gpt-5.3-codex-spark
        gpt-5.2 gpt-5.2-codex gpt-5.1 gpt-5.1-codex gpt-5.1-codex-max gpt-5.1-codex-mini
        gpt-5 gpt-5-codex gpt-5-nano grok-4.7 grok-4.6 grok-4.5 grok-build-0.1
        muse-spark-1.3 muse-spark-1.2 muse-spark-1.3-contributor-free]],
    gemini = [[gemini-3.8-flash gemini-3.7-flash gemini-3.6-flash gemini-3.5-flash
        gemini-3.5-flash-lite gemini-3.1-pro gemini-3-flash]],
})

function M.add(id, name, base_url, metadata_key, assignments)
    local apis = { chat = uji.api.openai(), messages = uji.api.anthropic(), responses = uji.api.responses(), gemini = uji.api.gemini() }
    for _, api in pairs(apis) do
        if api.headers then
            local headers = api.headers
            function api:headers(request, ...)
                local out = headers(self, request, ...)
                out["User-Agent"] = USER_AGENT
                if request.session and request.session ~= "" then
                    out["x-opencode-session"] = request.session
                end
                return out
            end
        end
    end
    local service = {}
    function service:efforts(model)
        local api = apis[assignments[model]]
        return api and api.efforts and api:efforts(model) or { "off", "minimal", "low", "medium", "high" }
    end
    function service:stream(request, reply)
        local provider = catalog.get(id)
        local ready, failure = provider:ensure()
        if not ready then
            return reply.fail(failure)
        end
        local api = apis[assignments[request.model]]
        if not api or not provider:model(request.model) then
            return reply.fail({
                kind = "provider",
                message = "unsupported or unavailable " .. name .. " model: " .. tostring(request.model),
            })
        end
        return api:stream(request, reply)
    end
    uji.provider.add({
        id = id,
        name = name,
        base_url = base_url,
        auth_env = { "OPENCODE_API_KEY" },
        api = service,
        discover = function(provider)
            local inventory = fetch(provider.base_url:gsub("/$", "") .. "/models")
            local document = fetch(METADATA_URL)
            local source = document[metadata_key]
            if
                type(inventory.data) ~= "table"
                or getmetatable(inventory.data) ~= getmetatable(sys.json.array({}))
                or type(source) ~= "table"
                or type(source.models) ~= "table"
            then
                error("invalid " .. name .. " inventory or metadata; run /models to retry", 0)
            end
            local found, seen, skipped = {}, {}, 0
            for _, item in ipairs(inventory.data) do
                if type(item) ~= "table" or type(item.id) ~= "string" or item.id == "" then
                    error("invalid " .. name .. " model ID", 0)
                end
                local entry = source.models[item.id]
                local row = type(entry) == "table" and assignments[item.id] and metadata(item.id, entry)
                if row and not seen[item.id] then
                    found[#found + 1], seen[item.id] = row, true
                elseif not row then
                    skipped = skipped + 1
                end
            end
            table.sort(found, function(a, b)
                return a.id < b.id
            end)
            if skipped > 0 then
                uji.notify(name .. ": excluded " .. skipped .. " models without supported routing or complete metadata")
            end
            return found
        end,
    })
end

return M
