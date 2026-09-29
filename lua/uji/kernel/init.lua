local exports = require("uji.kernel.exports")
local modules = require("uji.kernel.modules")
local scheduler = require("uji.kernel.scheduler")

local EXTRAS = {
    "uji.kernel.db",
    "uji.kernel.json",
    "uji.kernel.markdown",
    "uji.kernel.os",
    "uji.kernel.tty",
}

local kernel = {}

for name, value in pairs(uji) do
    kernel[name] = value
end

local function install(fields)
    for name, value in pairs(fields) do
        local existing = uji[name]
        if type(existing) == "table" and type(value) == "table" then
            for key, item in pairs(value) do
                existing[key] = item
            end
        else
            uji[name] = value
        end
        kernel[name] = uji[name]
    end
end

install(exports.functions)

install({
    task = {
        spawn = scheduler.spawn,
        race = scheduler.race,
        timeout = scheduler.timeout,
        on_error = scheduler.on_error,
    },
    sleep = scheduler.sleep,
    promise = scheduler.promise,
})

for _, extra in ipairs(EXTRAS) do
    install(require(extra))
end

kernel.native = modules

table.insert(package.searchers, modules.searcher)

return kernel
