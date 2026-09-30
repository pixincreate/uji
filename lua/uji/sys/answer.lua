local ffi = require("ffi")
local natives = require("uji.sys.native")
local scheduler = require("uji.sys.scheduler")

local VALUE, FAILED, END, LATE = 0, 1, 2, 3
local QUIET = { nulls = false }

local failure = natives.uji_error
local releases = setmetatable({}, { __mode = "k" })

local M = { VALUE = VALUE, FAILED = FAILED, END = END, LATE = LATE }

local function bytes(pointer, length)
    local size = tonumber(length)
    return size > 0 and ffi.string(pointer, size) or ""
end

function M.error()
    local answer = failure()
    local message = bytes(answer.data, answer.length)
    answer.free(answer)
    return message ~= "" and message or "a native function failed"
end

function M.read(answer)
    if answer == nil then
        return FAILED, M.error()
    end
    local status = answer.status
    local data = bytes(answer.data, answer.length)
    local handle
    if answer.handle ~= nil then
        handle = ffi.gc(answer.handle, answer.release)
        releases[handle] = answer.release
    end
    answer.free(answer)
    return status, data, handle
end

function M.value(answer, shape)
    local status, data, handle = M.read(answer)
    if status == FAILED then
        error(data, 2)
    elseif status == END then
        return nil
    elseif status == LATE then
        return false
    end
    if shape then
        return shape(data, handle)
    end
    return data
end

function M.settle(answer, shape)
    local status, data, handle = M.read(answer)
    if status == FAILED then
        return nil, data
    elseif status == END then
        return nil
    elseif status == LATE then
        return false
    end
    if shape then
        return shape(data, handle)
    end
    return data
end

function M.wait(token)
    local number = tonumber(token)
    if number == 0 then
        error(M.error(), 3)
    end
    return scheduler.await(number)
end

function M.await(token, shape)
    return M.settle(M.wait(token), shape)
end

function M.done()
    return true
end

local decode

function M.decoded(data)
    decode = decode or require("uji.sys.json").json.decode
    return decode(data, QUIET)
end

function M.release(handle)
    local release = handle ~= nil and releases[handle]
    if release then
        releases[handle] = nil
        ffi.gc(handle, nil)
        release(handle)
    end
end

return M
