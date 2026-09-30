local answer = require("uji.sys.answer")
local ffi = require("ffi")
local natives = require("uji.sys.native")

local parse, free = natives.markdown, natives.uji_release

local START, END, BREAK, RULE, TASK = 1, 2, 6, 7, 8
local KINDS = { "start", "end", "text", "code", "html", "break", "rule", "task" }
local TAGS = {
    "paragraph",
    "heading",
    "blockquote",
    "code_block",
    "list",
    "item",
    "emphasis",
    "strong",
    "strikethrough",
    "link",
    "image",
    "table",
    "table_head",
    "table_row",
    "table_cell",
    "other",
}

local out = ffi.new("uji_marks[1]")

local function details(mark, strings)
    local kind = mark.kind
    if kind == START then
        if mark.flag == 1 then
            return TAGS[mark.tag], ffi.string(strings + mark.offset, mark.length)
        elseif mark.flag == 2 then
            return TAGS[mark.tag], tonumber(mark.number)
        end
        return TAGS[mark.tag]
    elseif kind == END then
        return TAGS[mark.tag]
    elseif kind == BREAK then
        return mark.flag == 1 and "hard" or "soft"
    elseif kind == TASK then
        return mark.flag == 1
    elseif kind == RULE then
        return nil
    end
    return ffi.string(strings + mark.offset, mark.length)
end

local M = {}

function M.markdown(source)
    local handle = parse(source, #source, out)
    if handle == nil then
        error(answer.error(), 0)
    end
    local marks, strings = out[0].items, out[0].strings
    local events = {}
    for index = 0, tonumber(out[0].count) - 1 do
        local mark = marks[index]
        local first, second = details(mark, strings)
        events[index + 1] = { KINDS[mark.kind], first, second, tonumber(mark.start), tonumber(mark["end"]) }
    end
    free(handle)
    return events
end

return M
