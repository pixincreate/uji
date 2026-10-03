local common = require("uji.builtin.apis.common")
local defaults = require("uji.core.catalog").DEFAULT_EFFORTS
local sys = require("uji.sys")

local METADATA = "https://models.dev/api.json"
local SOURCES = { "opencode", "opencode-go" }
local AGENT = "uji/" .. require("uji.version")

local KNOWN = { none = "off" }
for _, effort in ipairs(require("uji.core.catalog").EFFORTS) do
    KNOWN[effort] = effort
end

local function client(Api)
    local Client = uji.class(Api)
    function Client:headers(request, ...)
        local out = Api.headers(self, request, ...)
        out["User-Agent"] = AGENT
        out["x-opencode-session"] = common.session(request)
        return out
    end
    return Client()
end

local CHAT = client(uji.api.openai)
local CLIENTS = {
    ["@ai-sdk/openai"] = client(uji.api.responses),
    ["@ai-sdk/anthropic"] = client(uji.api.anthropic),
    ["@ai-sdk/google"] = client(uji.api.gemini),
}

local function fetch(url)
    local response, err = uji.http.request({ url = url, headers = { ["User-Agent"] = AGENT }, timeout = 10 })
    if not response then
        error(err, 0)
    end
    if response.status ~= 200 then
        error(url .. " answered " .. response.status, 0)
    end
    return uji.json.decode(response.body, { nulls = false })
end

local pending

local function metadata()
    local mine = pending
    if not mine then
        mine = sys.promise()
        pending = mine
        local ok, value = pcall(function()
            local all, kept = fetch(METADATA), {}
            for _, source in ipairs(SOURCES) do
                kept[source] = all[source] and all[source].models or {}
            end
            return kept
        end)
        if not ok then
            pending = nil
        end
        mine:resolve(ok, value)
    end
    local ok, value = mine:await()
    if not ok then
        error(value, 0)
    end
    return value
end

local function positive(value)
    return type(value) == "number" and value > 0 and value or nil
end

local function efforts(options)
    for _, option in ipairs(type(options) == "table" and options or {}) do
        if type(option) == "table" and option.type == "effort" and type(option.values) == "table" then
            local out = {}
            for _, value in ipairs(option.values) do
                out[#out + 1] = KNOWN[value]
            end
            return #out > 0 and out or nil
        end
    end
end

local function images(info)
    local input = info.modalities and info.modalities.input
    if type(input) ~= "table" then
        return nil
    end
    for _, modality in ipairs(input) do
        if modality == "image" then
            return true
        end
    end
    return false
end

local function entry(id, info)
    if not info then
        return { id = id }
    end
    local limit = info.limit or {}
    return {
        id = id,
        context = positive(limit.context),
        output = positive(limit.output),
        reasoning = info.reasoning == true,
        images = images(info),
        cache = info.cost ~= nil and info.cost.cache_read ~= nil,
        efforts = efforts(info.reasoning_options),
    }
end

local OpenCode = uji.class()

function OpenCode:init()
    self.routes = {}
end

function OpenCode:route(model)
    return self.routes[model] or CHAT
end

function OpenCode:efforts(model)
    local api = self:route(model)
    return api.efforts and api:efforts(model) or defaults
end

function OpenCode:stream(request, reply)
    return self:route(request.model):stream(request, reply)
end

return function(spec)
    local api = OpenCode()
    uji.provider.add({
        id = spec.id,
        name = spec.name,
        api = api,
        base_url = spec.base_url,
        auth_env = { "OPENCODE_API_KEY" },
        models = function()
            local listed = fetch(spec.base_url .. "/models")
            if type(listed) ~= "table" or type(listed.data) ~= "table" then
                error(spec.base_url .. "/models sent no model list", 0)
            end
            local known = metadata()[spec.source]
            local out, seen = {}, {}
            for _, item in ipairs(listed.data) do
                local id = type(item) == "table" and item.id
                if type(id) == "string" and not seen[id] then
                    seen[id] = true
                    local info = known[id]
                    local sdk = info and info.provider and info.provider.npm
                    api.routes[id] = CLIENTS[sdk] or CHAT
                    out[#out + 1] = entry(id, info)
                end
            end
            table.sort(out, function(a, b)
                return a.id < b.id
            end)
            return out
        end,
    })
end
