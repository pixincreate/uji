# uji.provider

Use `/login` and `/models` to select built-in providers or providers you add here.

## uji.provider.add(spec)

Adds a provider, or merges `spec` into the provider with the same `id`.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | Required. The provider's id. |
| `name` | string | The name `/login` shows. Required for a new provider. |
| `api` | object | The API the provider speaks, such as `uji.api.openai()`. [Provider APIs](apis.md) lists the built-in ones and how to change them. Required for a new provider. |
| `base_url` | string | The API root, such as `"https://api.openai.com/v1"`. Required for a new provider. |
| `auth_env` | list of strings | Environment variables that may hold the API key. |
| `models` | list | Model ids, or tables with `id`, `context`, `output`, `reasoning`, `cache`, `images` and `efforts`. `images` is `true` or `false` when you know whether the model takes images. `efforts` lists the reasoning efforts the model accepts, from `off`, `minimal`, `low`, `medium`, `high`, `xhigh` and `max`. Without it, uji asks the provider's `api`. |
| `context_window` | integer | The context size to assume for a model that does not set one. |
| `oauth` | table | Subscription sign-in settings. The built-in Anthropic and OpenAI providers show the format. |
| `discover` | function | Optional remote model discovery callback. Receives `{ id, base_url }` and returns model tables, or `nil, failure`. |

When the provider exists, each field you give replaces the old one, except
`models`, which merge by `id`. A model whose `id` is already listed replaces
the old one whole.
For a provider with `discover`, model declarations override matching discovered IDs only.
They do not add undiscovered models to the usable catalog.

Raises an error for an unknown field, for an `api` without a `stream` method,
for an unknown effort, and for a new provider without `name`, `api` and
`base_url`.

```lua
uji.provider.add({
  id = "litellm",
  name = "LiteLLM",
  api = uji.api.openai(),
  base_url = "http://localhost:4000",
  auth_env = { "LITELLM_API_KEY" },
  models = {
    { id = "claude-sonnet-4-5", context = 200000, output = 64000, reasoning = true },
  },
})

uji.provider.add({ id = "openai", base_url = "https://proxy.example.com/v1" })
```

## Remote model discovery

Discovery-enabled providers start with an empty catalog.
Registration and `uji.provider.list()` do not make network requests.
Login, `/models`, and first inference load the selected provider's catalog.
Other providers remain unloaded until you select or explicitly refresh them.

A refresh replaces the previous catalog rather than merging it.
Concurrent refreshes share one discovery operation.
Failed refreshes leave an empty catalog and block inference; there is no bundled or stale fallback.
Run `/models` or `uji.provider.refresh(id)` to retry.
Unavailable saved model IDs produce an error instead of silently switching models.

The callback returns the same model-table fields accepted by `models`.
Return `nil, { kind = "provider", message = "..." }` on failure, or raise an error.
The catalog validates the returned rows before publishing them.
Changing a provider during discovery invalidates that result.

## OpenCode Zen and Go

Run `/login`, select **OpenCode Go**, and enter your OpenCode console API key.
You can also set `OPENCODE_API_KEY`.
Go uses `https://opencode.ai/zen/go/v1`; Zen uses `https://opencode.ai/zen/v1`.
Both fetch available IDs from their own `/models` endpoint and limits/capabilities from `https://models.dev/api.json`.
These public discovery requests carry no API key.
Only IDs present in both sources with a documented service route become usable.
Missing metadata and unknown routes exclude a model with a notice.
Metadata or inventory outages block discovery and inference.

Go routes through Chat Completions, Anthropic Messages, or Responses.
Zen also supports Gemini requests.
Routing is service-specific: Go's MiniMax and Qwen Max models use Messages, while their Zen counterparts use Chat Completions.
Client and owning-conversation headers apply to both services, including title and compaction requests.
Saved base-URL overrides are rejected for discovery providers; use `/login` to clear an old override.

Model availability and subscription allowances depend on your account.
Context and output token limits do not represent remaining subscription allowance.
Disable **Use balance** in the Go console if you do not want account-enabled pay-as-you-go fallback.
Go API-key access does not configure ChatGPT subscription authentication.
Inference rejection diagnostics include a bounded server message with outgoing API keys and control bytes removed.
HTTP 403 can indicate account, balance, region, or model restrictions; it does not establish that the key is invalid.

## uji.provider.refresh(id)

Refreshes the named provider and returns its public snapshot, or `nil, failure`.
Static providers return their existing snapshot without network requests.
Discovery failures use `{ kind = "provider", message = "..." }`.

```lua
local provider, failure = uji.provider.refresh("opencode-go")
if not provider then
  uji.notify(failure.message)
end
```

## uji.provider.remove(id)

Removes a provider and returns `true` if it existed.

```lua
uji.provider.remove("perplexity")
```

## uji.provider.list()

Returns one table per provider with `id`, `name`, `api`, `base_url`,
`auth_env`, `context_window` and `models`, and `oauth`, which is `true` when
the provider offers subscription sign-in. Each model has `id`, `context`,
`output`, `reasoning`, `cache`, `images` and `efforts`.
Discovery providers also expose `discovery`: `unloaded`, `loading`, `ready`, or `failed`.
This operation returns local state only.

```lua
for _, provider in ipairs(uji.provider.list()) do
  if provider.base_url:find("localhost", 1, true) then
    uji.notify(provider.name)
  end
end
```
