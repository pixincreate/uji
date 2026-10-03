local catalog = require("uji.core.catalog")
local task = require("uji.core.task")

local function model_row(entry)
    return {
        id = entry.id,
        context = entry.context,
        output = entry.output,
        reasoning = entry.reasoning,
        cache = entry.cache,
        images = entry.images,
        efforts = entry.efforts and { unpack(entry.efforts) },
    }
end

local function model_rows(provider)
    local rows = {}
    for index, entry in ipairs(provider.models) do
        rows[index] = model_row(entry)
    end
    return rows
end

local function provider_row(provider)
    return {
        id = provider.id,
        name = provider.name,
        api = provider.api,
        base_url = provider.base_url,
        auth_env = { unpack(provider.auth_env) },
        oauth = provider.oauth ~= nil,
        context_window = provider.context_window,
        models = model_rows(provider),
        state = provider.state,
    }
end

uji.provider = {
    add = catalog.add,
    remove = catalog.remove,
    list = function()
        local rows = {}
        for index, provider in ipairs(catalog.all()) do
            rows[index] = provider_row(provider)
        end
        return rows
    end,
    models = task.callback(function(id)
        local provider = catalog.get(id)
        if not provider then
            error("no provider is registered as " .. tostring(id), 3)
        end
        local _, failure = provider:load()
        return model_rows(provider), failure
    end),
}
