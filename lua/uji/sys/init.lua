local exports = require("uji.sys.exports")
local ffi = require("ffi")
local modules = require("uji.sys.modules")
local scheduler = require("uji.sys.scheduler")

local PLATFORMS = { OSX = "macos", Linux = "linux", Windows = "windows" }

local EXTRAS = {
    "uji.sys.db",
    "uji.sys.json",
    "uji.sys.markdown",
    "uji.sys.tty",
}

local runtime = {}

local function merge(fields)
    for name, value in pairs(fields) do
        local existing = runtime[name]
        if type(existing) == "table" and type(value) == "table" then
            for key, item in pairs(value) do
                existing[key] = item
            end
        else
            runtime[name] = value
        end
    end
end

merge(exports.functions)

merge({
    os = {
        platform = PLATFORMS[ffi.os] or "other",
        roots = UJI_NATIVE.roots,
        carry = UJI_NATIVE.carry,
    },
    task = {
        spawn = scheduler.spawn,
        race = scheduler.race,
        timeout = scheduler.timeout,
        on_error = scheduler.on_error,
    },
    sleep = scheduler.sleep,
    promise = scheduler.promise,
    native = modules,
})

for _, extra in ipairs(EXTRAS) do
    merge(require(extra))
end

table.insert(package.searchers, modules.searcher)

setmetatable(uji, { __index = runtime })

return runtime
