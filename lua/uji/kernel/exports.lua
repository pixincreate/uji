local answer = require("uji.kernel.answer")
local json = require("uji.kernel.json").json
local natives = require("uji.kernel.native")

local EMPTY = {}

local Exports = {}
Exports.__index = Exports

function Exports:class(name)
    local class = self.classes[name]
    if not class then
        class = {}
        class.__index = class
        self.classes[name] = class
    end
    return class
end

function Exports:object(name)
    local class = self:class(name)
    return function(data, handle)
        local fields = data ~= "" and answer.decoded(data) or nil
        if type(fields) ~= "table" then
            fields = {}
        end
        fields.handle = handle
        return setmetatable(fields, class)
    end
end

function Exports:place(path, value)
    local owner, method = path:match("^([%w_]+):([%w_]+)$")
    if owner then
        self:class(owner)[method] = value
        return
    end
    local prefix, name = path:match("^([%w_]+)%.([%w_]+)$")
    if not prefix then
        self.functions[path] = value
        return
    end
    self.functions[prefix] = self.functions[prefix] or {}
    self.functions[prefix][name] = value
end

local function numeric(data)
    return tonumber(data)
end

local function unpacked(data)
    return unpack(answer.decoded(data))
end

local function build(found, scripts)
    local exports = setmetatable({ classes = {}, functions = {} }, Exports)
    local env = {
        natives = found,
        value = answer.value,
        settle = answer.settle,
        wait = answer.wait,
        decoded = answer.decoded,
        done = answer.done,
        numeric = numeric,
        unpacked = unpacked,
        encode = json.encode,
        EMPTY = EMPTY,
        tonumber = tonumber,
        object = function(name)
            return exports:object(name)
        end,
    }
    for _, script in ipairs(scripts) do
        local make = assert(load(script.source, "=" .. script.place, "t", env))
        exports:place(script.place, make())
    end
    return exports
end

local kernel = build(natives, UJI_NATIVE.wrappers)
kernel.build = build

return kernel
