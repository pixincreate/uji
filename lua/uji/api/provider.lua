local catalog = require("uji.catalog")

local function provider_row(provider)
    local models = {}
    for index, entry in ipairs(provider.models) do
        models[index] = { id = entry.id, context = entry.context, output = entry.output }
    end
    return {
        id = provider.id,
        name = provider.name,
        wire = provider.wire,
        base_url = provider.base_url,
        models = models,
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
}
