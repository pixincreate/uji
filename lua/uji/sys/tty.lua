local answer = require("uji.sys.answer")
local exports = require("uji.sys.exports")

local EDGE = 65535

local Screen = exports:class("screen")
local open = exports.functions.tty.open
local put = Screen.put

local function placed(at)
    if at < 0 then
        error(answer.error(), 3)
    end
    return at
end

function Screen:line(row, col, spans, width)
    local stop = width and math.min(col + width, EDGE) or EDGE
    if spans == nil then
        return col
    end
    if type(spans) == "string" then
        return placed(put(self, row, col, stop, spans, 0))
    end
    if type(spans) ~= "table" then
        error("a line is a string or a list of spans, not a " .. type(spans), 2)
    end
    local at = col
    for _, span in ipairs(spans) do
        if type(span) == "string" then
            at = placed(put(self, row, at, stop, span, 0))
        elseif type(span) == "table" then
            at = placed(put(self, row, at, stop, span[1], span[2] or 0))
        else
            error("a span is a string or a table, not a " .. type(span), 2)
        end
    end
    return at
end

return {
    tty = {
        open = function()
            return setmetatable({}, Screen), open()
        end,
    },
}
