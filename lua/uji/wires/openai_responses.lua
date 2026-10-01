local common = require("uji.wires.common")
local stream = require("uji.wires.stream")
local sys = require("uji.sys")
local wire = require("uji.wire")

local M = {}

local function binding(request)
    return sys.base64.encode(sys.sha256(request.auth.key), { url = true, pad = false })
end

local function picture(image)
    return { type = "input_image", image_url = "data:" .. image.media_type .. ";base64," .. image.data }
end

local function text(value)
    return { type = "input_text", text = value }
end

function M.encode(request)
    assert(type(request.auth.key) == "string" and request.auth.key ~= "", "Responses requires an API key")
    local input = sys.json.array({})
    local function push(item)
        input[#input + 1] = item
    end
    for _, message in ipairs(request.messages) do
        if message.type == "user" then
            push({ role = "user", content = common.parts(message.images, picture, message.text, text) })
        elseif message.type == "assistant" then
            local state = message.wire_state
            if state and state.wire == "openai-responses" and state.base_url == request.provider.base_url
                and state.provider == request.provider.id then
                assert(state.binding == binding(request), "Responses history belongs to a different API key")
                for _, item in ipairs(sys.json.decode(state.output)) do
                    push(item)
                end
            else
                if message.text ~= "" then
                    push({ role = "assistant", content = message.text })
                end
                for _, call in ipairs(message.tool_calls or {}) do
                    push({ type = "function_call", call_id = call.id, name = call.name, arguments = call.arguments })
                end
            end
        elseif message.type == "tool" then
            push({ type = "function_call_output", call_id = message.tool_call_id, output = message.content })
            if message.images then
                push({ role = "user", content = common.parts(message.images, picture, common.SHOWN, text) })
            end
        end
    end
    local compat = request.provider.compat or {}
    return {
        model = request.model,
        instructions = common.system(request),
        input = input,
        tools = common.nonempty(common.map(request.tools, function(spec)
            return { type = "function", name = spec.name, description = spec.description,
                parameters = spec.parameters, strict = false }
        end)),
        stream = true,
        store = false,
        include = sys.json.array({ "reasoning.encrypted_content" }),
        reasoning = request.effort ~= "off" and { effort = request.effort, summary = "auto" } or nil,
        max_output_tokens = compat.max_tokens_field ~= "none" and request.max_output or nil,
        prompt_cache_key = request.cache ~= "off" and request.session or nil,
    }
end

function M.decode(response, request)
    if response.status ~= "completed" then
        local detail = type(response.error) == "table" and response.error or response.incomplete_details
        return nil, { kind = "provider", message = type(detail) == "table"
            and (detail.message or detail.reason or "Responses reply did not complete") or "Responses reply did not complete" }
    end
    if type(response.output) ~= "table" then
        return nil, { kind = "provider", message = "Responses reply has no output items" }
    end
    local written, reasoning, calls = {}, {}, {}
    for _, item in ipairs(response.output) do
        if item.type == "message" then
            for _, part in ipairs(item.content or {}) do
                if part.type == "output_text" then
                    written[#written + 1] = assert(type(part.text) == "string" and part.text, "invalid output text")
                elseif part.type == "refusal" then
                    return nil, { kind = "provider", message = part.refusal or "model refused the request" }
                end
            end
        elseif item.type == "reasoning" then
            for _, part in ipairs(item.summary or {}) do
                if part.type == "summary_text" then
                    reasoning[#reasoning + 1] = part.text
                end
            end
        elseif item.type == "function_call" then
            assert(type(item.call_id) == "string" and item.call_id ~= "", "missing function call ID")
            assert(type(item.name) == "string" and item.name ~= "", "missing function name")
            assert(type(item.arguments) == "string", "missing function arguments")
            local args = sys.json.decode(item.arguments)
            assert(type(args) == "table" and getmetatable(args) ~= getmetatable(sys.json.array({})),
                "function arguments must be a JSON object")
            calls[#calls + 1] = { id = item.call_id, name = item.name, arguments = item.arguments }
        else
            return nil, { kind = "provider", message = "unsupported Responses output item: " .. tostring(item.type) }
        end
    end
    if #written == 0 and #calls == 0 then
        return nil, stream.empty_response()
    end
    local reported = response.usage
    local usage
    if type(reported) == "table" then
        local read = reported.input_tokens_details and reported.input_tokens_details.cached_tokens or 0
        usage = { input = math.max((reported.input_tokens or 0) - read, 0), output = reported.output_tokens or 0,
            cache_read = read, cache_write = 0 }
    end
    return {
        text = table.concat(written),
        reasoning = #reasoning > 0 and table.concat(reasoning) or nil,
        tool_calls = calls,
        usage = usage,
        -- Keep opaque items encoded so transcript decoding cannot drop JSON nulls.
        wire_state = { wire = "openai-responses", provider = request.provider.id,
            base_url = request.provider.base_url, binding = binding(request), output = sys.json.encode(response.output) },
    }
end

function M.stream(request, reply)
    local ok, body = pcall(M.encode, request)
    if not ok then
        local failure = { kind = "provider", message = tostring(body) }
        reply.fail(failure)
        return nil, failure
    end
    local compat = request.provider.compat or {}
    local headers = { Authorization = common.bearer(request.auth.key), ["User-Agent"] = compat.user_agent or "uji" }
    if compat.session_header and request.session and request.session ~= "" then
        headers[compat.session_header] = request.session
    end
    return stream.run({
        url = request.provider.base_url .. "/responses",
        headers = headers,
        body = body,
        strict = true,
        json = {},
        read = function(event, parts)
            if event.type == "response.output_text.delta" then
                parts:push_text(event.delta)
            elseif event.type == "response.reasoning_summary_text.delta" then
                parts:push_reasoning(event.delta)
            elseif event.type == "response.function_call_arguments.delta"
                or event.type == "response.output_item.added" or event.type == "response.output_item.done" then
                -- Keep active tool generation alive; execute only the validated final calls.
                parts.moved = true
            elseif event.type == "response.completed" then
                local answer, failure = M.decode(event.response, request)
                if not answer then
                    parts.failure = failure
                else
                    parts.text = { answer.text }
                    parts.reasoning = { answer.reasoning or "" }
                    parts.calls = {}
                    for index, call in ipairs(answer.tool_calls) do
                        call.index = index
                        parts.calls[index] = call
                    end
                    parts.usage = answer.usage or parts.usage
                    parts.wire_state = answer.wire_state
                end
                parts.response_complete = true
                parts:finish()
            elseif event.type == "response.failed" or event.type == "response.incomplete" then
                local _, failure = M.decode(event.response, request)
                parts.failure = failure or { kind = "provider", message = "Responses reply did not complete" }
                parts:finish()
            elseif event.type == "error" then
                parts.failure = { kind = "provider", message = event.message or "Responses stream error" }
                parts:finish()
            end
        end,
        finished = function(parts)
            if not parts.response_complete then
                return stream.truncated_stream()
            end
        end,
    }, reply)
end

wire.add("openai-responses", M)

return M
