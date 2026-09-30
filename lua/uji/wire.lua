local Registry = require("uji.registry")
local check = require("uji.check")
local plugin = require("uji.plugin")

local M = { registry = plugin.track(Registry(plugin.current)) }

function M.add(name, spec)
    check.name(name, "uji.wire.add")
    if type(spec) ~= "table" or type(spec.stream) ~= "function" then
        error("wire " .. name .. " needs a stream function", 2)
    end
    return M.registry:add(name, spec)
end

function M.remove(name)
    return M.registry:remove(name)
end

function M.list()
    return M.registry:sorted()
end

function M.get(name)
    return M.registry:get(name)
end

return M
