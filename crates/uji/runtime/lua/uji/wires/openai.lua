local stream = require("uji.wires.stream")

local KNOB = {
    effort = function(body, level)
        body.reasoning_effort = level
    end,
    nested_effort = function(body, level)
        body.reasoning = level and { effort = level }
    end,
    switch = function(body, level)
        body.thinking = { type = level and "enabled" or "disabled" }
    end,
    flag = function(body, level)
        body.enable_thinking = level ~= nil
    end,
}

local THINKING = {
    openai = { KNOB.effort },
    openrouter = { KNOB.nested_effort },
    deepseek = { KNOB.switch, KNOB.effort },
    zai = { KNOB.switch },
    qwen = { KNOB.flag, KNOB.effort },
    none = {},
}

local function guess(base_url)
    local url = base_url:lower()
    local function has(needle)
        return url:find(needle, 1, true) ~= nil
    end
    local thinking = "openai"
    if has("openrouter.ai") then
        thinking = "openrouter"
    elseif has("deepseek.com") then
        thinking = "deepseek"
    elseif has("bigmodel.cn") or has("z.ai") then
        thinking = "zai"
    elseif has("dashscope") then
        thinking = "qwen"
    end
    return {
        max_tokens_field = has("api.openai.com") and "max_completion_tokens" or "max_tokens",
        thinking = thinking,
        tool_result_name = false,
        finish_reason = true,
    }
end

local function compat(provider)
    local resolved = guess(provider.base_url)
    for key, value in pairs(provider.compat or {}) do
        resolved[key] = value
    end
    return resolved
end

local function call(tool_call)
    return {
        id = tool_call.id,
        type = "function",
        ["function"] = {
            name = tool_call.name,
            arguments = uji.json.encode(stream.arguments(tool_call.arguments)),
        },
    }
end

local function message(item, resolved)
    if item.type == "user" or item.type == "system" then
        return { role = item.type, content = item.text }
    elseif item.type == "assistant" then
        local calls = {}
        for _, tool_call in ipairs(item.tool_calls or {}) do
            calls[#calls + 1] = call(tool_call)
        end
        return {
            role = "assistant",
            content = item.text ~= "" and item.text or uji.json.null,
            tool_calls = #calls > 0 and calls or nil,
        }
    elseif item.type == "tool" then
        return {
            role = "tool",
            content = item.content,
            tool_call_id = item.tool_call_id,
            name = resolved.tool_result_name and item.name or nil,
        }
    end
end

local function body(request, resolved)
    local messages = {}
    if request.system then
        messages[1] = { role = "system", content = request.system }
    end
    for _, item in ipairs(request.messages) do
        messages[#messages + 1] = message(item, resolved)
    end
    local tools = {}
    for _, tool in ipairs(request.tools) do
        tools[#tools + 1] = {
            type = "function",
            ["function"] = { name = tool.name, description = tool.description, parameters = tool.parameters },
        }
    end
    local out = {
        model = request.model,
        messages = messages,
        tools = #tools > 0 and tools or nil,
        stream = true,
        stream_options = { include_usage = true },
    }
    local limit = stream.fit_thinking(request.effort, request.max_output)
    local level = request.effort ~= "off" and request.effort or nil
    for _, knob in ipairs(THINKING[resolved.thinking] or {}) do
        knob(out, level)
    end
    if resolved.max_tokens_field ~= "none" then
        out[resolved.max_tokens_field] = limit
    end
    return out
end

local function usage(reported)
    local details = reported.prompt_tokens_details or {}
    local cache_read =
        math.max(details.cached_tokens or 0, reported.prompt_cache_hit_tokens or 0, reported.cached_tokens or 0)
    local cache_write = details.cache_write_tokens or 0
    return {
        input = math.max((reported.prompt_tokens or 0) - cache_read - cache_write, 0),
        output = reported.completion_tokens or 0,
        cache_read = cache_read,
        cache_write = cache_write,
    }
end

local function read(event, parts)
    local choice = event.choices and event.choices[1]
    local delta = choice and choice.delta or {}
    if delta.reasoning_content then
        parts:push_reasoning(delta.reasoning_content)
    end
    if delta.content then
        parts:push_text(delta.content)
    end
    if choice and choice.finish_reason then
        parts:finish(choice.finish_reason)
    end
    if event.usage then
        parts.usage = usage(event.usage)
    end
    for _, piece in ipairs(delta.tool_calls or {}) do
        local entry = parts:call(piece.index or 0)
        if piece.id then
            entry.id = piece.id
        end
        local fn = piece["function"]
        if fn and fn.name then
            entry.name = fn.name
        end
        if fn and fn.arguments then
            entry.arguments = entry.arguments .. fn.arguments
        end
    end
end

local function settle(parts, calls)
    if #calls > 0 then
        return nil
    end
    if parts.finish_reason == "length" then
        return stream.output_limit()
    end
    if not parts:has_text() then
        return stream.empty_response()
    end
end

return {
    stream = function(request, reply)
        local resolved = compat(request.provider)
        local key = request.auth.key
        return stream.run({
            url = request.provider.base_url .. "/chat/completions",
            headers = { Authorization = key and "Bearer " .. key },
            body = body(request, resolved),
            read = read,
            finished = function(parts)
                if not parts.complete and resolved.finish_reason then
                    return stream.truncated_stream()
                end
            end,
            settle = settle,
        }, reply)
    end,
}
