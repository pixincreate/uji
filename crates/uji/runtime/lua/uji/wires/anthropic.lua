local stream = require("uji.wires.stream")

local VERSION = "2023-06-01"
local MAX_TOKENS = "max_tokens"
local TTL = { long = "1h" }

local function text(value)
    return { type = "text", text = value }
end

local function translate(request)
    local system = request.system or ""
    local messages = {}
    for _, item in ipairs(request.messages) do
        if item.type == "user" then
            messages[#messages + 1] = { role = "user", content = { text(item.text) } }
        elseif item.type == "assistant" then
            local blocks = uji.json.array({})
            if item.text ~= "" then
                blocks[#blocks + 1] = text(item.text)
            end
            for _, call in ipairs(item.tool_calls or {}) do
                blocks[#blocks + 1] =
                    { type = "tool_use", id = call.id, name = call.name, input = stream.arguments(call.arguments) }
            end
            messages[#messages + 1] = { role = "assistant", content = blocks }
        elseif item.type == "tool" then
            messages[#messages + 1] = {
                role = "user",
                content = { { type = "tool_result", tool_use_id = item.tool_call_id, content = item.content } },
            }
        elseif item.type == "system" then
            system = system ~= "" and system .. "\n" .. item.text or item.text
        end
    end
    return system, messages
end

local function cache(body, retention)
    if retention == "off" then
        return
    end
    local control = { type = "ephemeral", ttl = TTL[retention] }
    local function mark(block)
        if block then
            block.cache_control = control
        end
    end
    mark(body.system[#body.system])
    mark(body.tools and body.tools[#body.tools])
    for at = #body.messages, 1, -1 do
        local message = body.messages[at]
        if message.role == "user" then
            mark(message.content[#message.content])
            return
        end
    end
end

local function body(request)
    local system, messages = translate(request)
    local tools = {}
    for _, tool in ipairs(request.tools) do
        tools[#tools + 1] = { name = tool.name, description = tool.description, input_schema = tool.parameters }
    end
    local max_tokens, budget = stream.fit_thinking(request.effort, request.max_output)
    local out = {
        model = request.model,
        max_tokens = max_tokens,
        thinking = budget > 0 and { type = "enabled", budget_tokens = budget } or nil,
        system = system ~= "" and { text(system) } or uji.json.array({}),
        messages = messages,
        stream = true,
        tools = #tools > 0 and tools or nil,
    }
    cache(out, request.cache)
    local identity = request.auth.oauth and request.auth.oauth.identity_prompt
    if identity and not (out.system[1] and out.system[1].text == identity) then
        table.insert(out.system, 1, text(identity))
    end
    return out
end

local function headers(auth)
    local out = { ["anthropic-version"] = VERSION }
    if auth.oauth then
        out.Authorization = "Bearer " .. auth.oauth.token
        for name, value in pairs(auth.oauth.headers or {}) do
            out[name] = value
        end
    elseif auth.key then
        out["x-api-key"] = auth.key
    end
    return out
end

local function block_delta(event, kind)
    if event.type == "content_block_delta" and event.delta and event.delta.type == kind then
        return event.delta
    end
end

local function read(event, parts)
    local thinking = block_delta(event, "thinking_delta")
    if thinking and thinking.thinking then
        parts:push_reasoning(thinking.thinking)
    end
    local reply = block_delta(event, "text_delta")
    if reply and reply.text then
        parts:push_text(reply.text)
    end
    local input = event.message and event.message.usage
    if input then
        parts.usage = {
            input = input.input_tokens or 0,
            output = parts.usage.output,
            cache_read = input.cache_read_input_tokens or 0,
            cache_write = input.cache_creation_input_tokens or 0,
        }
    end
    if event.usage then
        parts.usage.output = event.usage.output_tokens or 0
    end
    if event.delta and event.delta.stop_reason == MAX_TOKENS then
        parts.hit_limit = true
    end
    if event.type == "message_stop" then
        parts:finish()
    end
    if not event.index then
        return
    end
    local block = event.content_block
    if event.type == "content_block_start" and block and block.type == "tool_use" then
        local entry = parts:call(event.index)
        entry.id = block.id or ""
        entry.name = block.name or ""
    end
    local fragment = block_delta(event, "input_json_delta")
    if fragment and fragment.partial_json then
        local entry = parts:call(event.index)
        entry.arguments = entry.arguments .. fragment.partial_json
    end
end

return {
    stream = function(request, reply)
        return stream.run({
            url = request.provider.base_url .. "/messages",
            headers = headers(request.auth),
            body = body(request),
            read = read,
        }, reply)
    end,
}
