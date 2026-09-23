local M = {}

local MAX_ERROR_BODY = 2000
local EVENTS = { nulls = false }

function M.clip(text, max)
    local chars = utf8.len(text)
    local count = chars or #text
    if count <= max then
        return text
    end
    local cut = chars and utf8.offset(text, max + 1) or max + 1
    return string.format("%s\n… [truncated, %d of %d chars shown]", text:sub(1, cut - 1), max, count)
end

local function trim(text)
    return (text:gsub("^%s+", ""):gsub("%s+$", ""))
end

local function provider(message)
    return { kind = "provider", message = message }
end

function M.truncated_stream()
    return { kind = "http", message = "the provider closed the stream before the reply finished" }
end

function M.output_limit()
    return provider("response hit the model's output limit and was cut off")
end

function M.empty_response()
    return provider("empty response")
end

local function truncated_call(name)
    return provider(
        "the reply was cut off while calling `" .. name .. "`, so its arguments are incomplete - raise "
            .. "the model's max output"
    )
end

local function status_error(response)
    local status = response.status
    if status == 401 or status == 403 then
        return { kind = "auth", status = status }
    end
    local wait = response.headers["retry-after"]
    return {
        kind = "http",
        status = status,
        retry_after = wait and tonumber(wait:match("^%s*(%d+)%s*$")),
        message = M.clip(trim(response.body), MAX_ERROR_BODY),
    }
end

local function incomplete(arguments)
    if arguments:match("^%s*$") then
        return false
    end
    return not pcall(uji.json.decode, arguments)
end

local Parts = {}
Parts.__index = Parts

local function parts(reply)
    return setmetatable({
        reply = reply,
        text = {},
        reasoning = {},
        usage = { input = 0, output = 0, cache_read = 0, cache_write = 0 },
        hit_limit = false,
        complete = false,
        calls = {},
        moved = false,
    }, Parts)
end

function Parts:push_text(delta)
    self.text[#self.text + 1] = delta
    self.reply.text(delta)
    self.moved = true
end

function Parts:push_reasoning(delta)
    self.reasoning[#self.reasoning + 1] = delta
    self.reply.reasoning(delta)
    self.moved = true
end

function Parts:finish(reason)
    if reason then
        self.finish_reason = reason
    end
    self.complete = true
    self.moved = true
end

function Parts:call(index)
    self.moved = true
    for _, call in ipairs(self.calls) do
        if call.index == index then
            return call
        end
    end
    local call = { index = index, id = "", name = "", arguments = "" }
    self.calls[#self.calls + 1] = call
    return call
end

function Parts:progress()
    local moved = self.moved
    self.moved = false
    return moved
end

function Parts:has_text()
    return table.concat(self.text) ~= ""
end

function Parts:tool_calls()
    table.sort(self.calls, function(a, b)
        return a.index < b.index
    end)
    local calls = {}
    for _, call in ipairs(self.calls) do
        if incomplete(call.arguments) then
            return nil, truncated_call(call.name)
        end
        calls[#calls + 1] = { id = call.id, name = call.name, arguments = call.arguments }
    end
    return calls
end

function Parts:answer(calls)
    local usage = self.usage
    local total = usage.input + usage.output + usage.cache_read + usage.cache_write
    local reasoning = table.concat(self.reasoning)
    return {
        text = table.concat(self.text),
        reasoning = reasoning ~= "" and reasoning or nil,
        tool_calls = calls,
        usage = total > 0 and usage or nil,
    }
end

local function finished(parts)
    if not parts.complete then
        return M.truncated_stream()
    end
end

local function settle(parts, calls)
    if #calls == 0 and parts.hit_limit then
        return M.output_limit()
    end
end

function M.run(spec, reply)
    local state = parts(reply)
    local headers = { ["Content-Type"] = "application/json" }
    for name, value in pairs(spec.headers) do
        headers[name] = value
    end
    local id = uji.http.request({
        url = spec.url,
        method = "POST",
        headers = headers,
        body = uji.json.encode(spec.body),
        idle = M.idle,
        on_line = function(line)
            local data = line:match("^data: (.*)$")
            if not data then
                return false
            end
            if data == "[DONE]" then
                state:finish()
            else
                local ok, event = pcall(uji.json.decode, data, EVENTS)
                if ok and type(event) == "table" then
                    pcall(spec.read, event, state)
                end
            end
            return state:progress()
        end,
    }, function(response, err)
        if not response then
            return reply.fail({ kind = "http", message = err })
        end
        if response.status < 200 or response.status >= 300 then
            return reply.fail(status_error(response))
        end
        local problem = (spec.finished or finished)(state)
        if problem then
            return reply.fail(problem)
        end
        local calls, cut = state:tool_calls()
        if not calls then
            return reply.fail(cut)
        end
        problem = (spec.settle or settle)(state, calls)
        if problem then
            return reply.fail(problem)
        end
        reply.done(state:answer(calls))
    end)
    return function()
        uji.http.cancel(id)
    end
end

return M
