local answer = require("uji.sys.answer")
local buffer = require("string.buffer")
local ffi = require("ffi")
local natives = require("uji.sys.native")

local tokenize, free = natives.json_tokens, natives.uji_release
local find, gsub, format = string.find, string.gsub, string.format

local FALSE, TRUE, NUMBER, STRING, ARRAY, OBJECT = 1, 2, 3, 4, 5, 6
local SEQUENCE = {}
local NULL = newproxy(false)
local LARGEST = 2 ^ 53
local SPECIAL = '[%c"\\]'
local DIGITS = { "%.14g", "%.15g", "%.16g", "%.17g" }
local ESCAPES = { ['"'] = '\\"', ["\\"] = "\\\\", ["\b"] = "\\b", ["\f"] = "\\f", ["\n"] = "\\n", ["\r"] = "\\r", ["\t"] = "\\t" }

for code = 0, 31 do
    local char = string.char(code)
    ESCAPES[char] = ESCAPES[char] or format("\\u%04x", code)
end

local function mark(list)
    return setmetatable(list, SEQUENCE)
end

local out = ffi.new("uji_tokens[1]")
local tokens, strings, at, nulls

local function text(token)
    return ffi.string(strings + token.offset, token.length)
end

local value

local function object(count)
    local built = {}
    for _ = 1, count do
        local key = text(tokens[at])
        at = at + 1
        built[key] = value()
    end
    return built
end

local function array(count)
    local built = {}
    for index = 1, count do
        built[index] = value()
    end
    return mark(built)
end

value = function()
    local token = tokens[at]
    at = at + 1
    local kind = token.kind
    if kind == STRING then
        return text(token)
    elseif kind == NUMBER then
        return token.number
    elseif kind == OBJECT then
        return object(tonumber(token.count))
    elseif kind == ARRAY then
        return array(tonumber(token.count))
    elseif kind == TRUE then
        return true
    elseif kind == FALSE then
        return false
    end
    if nulls then
        return NULL
    end
    return nil
end

local encoded = buffer.new()
local visiting
local write

local function number(value)
    if value ~= value or value == math.huge or value == -math.huge then
        encoded:put("null")
    elseif value % 1 == 0 and value >= -LARGEST and value <= LARGEST then
        encoded:putf("%d", value)
    else
        for _, digits in ipairs(DIGITS) do
            local text = format(digits, value)
            if tonumber(text) == value or digits == DIGITS[#DIGITS] then
                encoded:put(text)
                return
            end
        end
    end
end

local function quoted(value)
    if find(value, SPECIAL) then
        value = gsub(value, SPECIAL, ESCAPES)
    end
    encoded:put('"', value, '"')
end

local function sequence(value, length)
    encoded:put("[")
    for index = 1, length do
        if index > 1 then
            encoded:put(",")
        end
        write(value[index])
    end
    encoded:put("]")
end

local function object(value)
    encoded:put("{")
    local first = true
    for key, item in pairs(value) do
        local kind = type(key)
        if kind ~= "string" and kind ~= "number" then
            error("json.encode needs string keys, not a " .. kind, 0)
        end
        if not first then
            encoded:put(",")
        end
        first = false
        quoted(tostring(key))
        encoded:put(":")
        write(item)
    end
    encoded:put("}")
end

local function tabled(value)
    if visiting[value] then
        error("json.encode found a table inside itself", 0)
    end
    visiting[value] = true
    local length = #value
    if length > 0 or getmetatable(value) == SEQUENCE then
        sequence(value, length)
    else
        object(value)
    end
    visiting[value] = nil
end

write = function(value)
    local kind = type(value)
    if kind == "string" then
        quoted(value)
    elseif kind == "number" then
        number(value)
    elseif kind == "table" then
        tabled(value)
    elseif kind == "boolean" then
        encoded:put(value and "true" or "false")
    elseif value == nil or value == NULL then
        encoded:put("null")
    else
        error("json.encode cannot encode a " .. kind, 0)
    end
end

local M = { json = { array = mark, null = NULL } }

function M.json.encode(value)
    encoded:reset()
    visiting = {}
    local ok, err = pcall(write, value)
    if not ok then
        error(err, 2)
    end
    return encoded:tostring()
end

function M.json.decode(source, opts)
    if type(source) ~= "string" then
        error("json.decode needs a string, not a " .. type(source), 2)
    end
    local handle = tokenize(source, #source, out)
    if handle == nil then
        error(answer.error(), 0)
    end
    tokens, strings, at = out[0].items, out[0].strings, 0
    nulls = not (type(opts) == "table" and opts.nulls == false)
    local ok, result = pcall(value)
    free(handle)
    if not ok then
        error(result, 0)
    end
    return result
end

return M
