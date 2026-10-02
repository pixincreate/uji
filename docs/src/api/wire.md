# uji.wire

Wires translate model calls into an API's HTTP format and stream the answer back.
Uji ships `openai-chat`, `openai-responses`, `anthropic` and `gemini` as Lua modules in `uji.wires`.
Use one of those modules as a reference when you add a wire.

## uji.wire.add(name, spec)

Registers a wire, or replaces the wire with the same name. `spec.stream` is a
function that receives a request and a reply table. It may return a function
that cancels the call.

The request has these fields:
- `model`, the model id
- `system`, the system prompt
- `messages`, the conversation in uji's message format, where a user or tool
  message may have `images`, a list of tables with `media_type`, base64
  `data`, `name`, `width` and `height`
- `tools`, a list of `name`, `description` and `parameters`
- `effort`, one of `off`, `minimal`, `low`, `medium` and `high`
- `max_output`, the output token limit
- `cache`, one of `off`, `short` and `long`
- `session`, the session id
- `provider`, with `id`, `base_url` and `compat`
- `auth`, with `key`, and `oauth` for subscription sign-in

The reply table has four functions:
- `reply.text(delta)` streams answer text.
- `reply.reasoning(delta)` streams reasoning text.
- `reply.done(answer)` finishes the call. `answer` has `text`, and optionally
  `reasoning`, `tool_calls`, `usage` and `wire_state`. Each tool call has `id`, `name` and
  `arguments`, where `arguments` is a JSON string. `usage` has `input`,
  `output`, `cache_read` and `cache_write`.
- `reply.fail(failure)` ends the call with an error. `failure.kind` is
  `"http"`, with `status`, `message` and `retry_after`, or `"auth"`, with
  `status`, or `"provider"`, with `message`. uji retries `http` failures with a
  status of 408, 409, 425, 429 or 5xx, and `http` failures with no status,
  such as a dropped connection.

`wire_state` is optional, JSON-serializable state owned by a wire.
Uji retains it on assistant messages through tool continuations and session reloads.
Other wires can ignore it.
Do not put credentials in this state.

## OpenAI Responses

Use `openai-responses` for API-key endpoints that accept the OpenAI Responses format.
It sends `POST <base_url>/responses`, not Chat Completions.
It supports text, images, function tools, reasoning summaries and token usage.
It requests encrypted reasoning items and replays the ordered output items with `store: false`.
Retained items are bound to the provider, endpoint and API-key fingerprint.
If you change that key, start a different conversation rather than replaying the old state.
Native prefix compaction replaces older items with a summary, as it does for other wires.
This wire does not implement ChatGPT subscription authentication.

For example, configure a Go Responses model separately:

```lua
uji.provider.add({
  id = "opencode-go-responses",
  name = "OpenCode Go Responses",
  wire = "openai-responses",
  base_url = "https://opencode.ai/zen/go/v1",
  auth_env = { "OPENCODE_API_KEY" },
  compat = { user_agent = "uji", session_header = "x-opencode-session" },
  models = {
    { id = "muse-spark-1.3-contributor", context = 1048576, output = 131072, reasoning = true },
  },
})
```

The wire accepts `compat.user_agent` (default `"uji"`), `compat.session_header` (unset by default), and `compat.max_tokens_field = "none"` to omit the output limit.
The codec module `uji.wires.openai_responses` exposes `encode(request)` and `decode(response, request)` for request and completed-response translation.
`encode` raises for invalid history or missing API-key authentication.
`decode` returns an answer or a provider failure; malformed output items raise.
Streaming converts decoder exceptions to provider failures and rejects incomplete or truncated replies before running tools.

## Custom wire example

```lua
uji.wire.add("echo", {
  stream = function(request, reply)
    local last = request.messages[#request.messages]
    reply.text("you said: ")
    reply.done({ text = "you said: " .. (last.text or "") })
  end,
})
```

## uji.wire.remove(name)

Removes a wire and returns `true` if it existed.

```lua
uji.wire.remove("echo")
```

## uji.wire.list()

Returns the names of the registered wires.

```lua
local wires = uji.wire.list()
```
