# uji.provider

uji ships 22 providers, and `/login` and `/models` offer every one you add
here as well.

## uji.provider.add(spec)

Adds a provider, or merges `spec` into the provider with the same `id`.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | Required. The provider's id. |
| `name` | string | The name `/login` shows. Required for a new provider. |
| `api` | object | The API the provider speaks, such as `uji.api.openai()`. [Provider APIs](apis.md) lists the built-in ones and how to change them. Required for a new provider. |
| `base_url` | string | The API root, such as `"https://api.openai.com/v1"`. Required for a new provider. |
| `auth_env` | list of strings | Environment variables that may hold the API key. |
| `models` | list or function | Model ids, or tables with `id`, `context`, `output`, `reasoning`, `cache`, `images` and `efforts`. `images` is `true` or `false` when you know whether the model takes images. `efforts` lists the reasoning efforts the model accepts, from `off`, `minimal`, `low`, `medium`, `high`, `xhigh` and `max`. Without it, uji asks the provider's `api`. A function returns that list, and uji calls it once, the first time it needs the provider's models. The function may wait, for example on `uji.http.request`. |
| `context_window` | integer | The context size to assume for a model that does not set one. |
| `oauth` | table | Subscription sign-in settings. The built-in Anthropic and OpenAI providers show the format. |

When the provider exists, each field you give replaces the old one, except
`models`, which merge by `id`. A model whose `id` is already listed replaces
the old one whole.

Raises an error for an unknown field, for an `api` without a `stream` method,
for `models` that are neither a list nor a function, for an unknown effort,
and for a new provider without `name`, `api` and `base_url`.

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

uji.provider.add({
  id = "ollama",
  models = function()
    local response = uji.http.request({ url = "http://localhost:11434/api/tags", timeout = 5 })
    local models = {}
    for _, entry in ipairs(uji.json.decode(response.body).models) do
      models[#models + 1] = entry.name
    end
    return models
  end,
})
```

uji calls the function when the provider is the current one, when `/models`
lists it, and before the first request to it. The models it returns merge
with the ones the provider already has. When it raises an error, the provider
keeps its models, and uji calls it again the next time it needs them.

## uji.provider.remove(id)

Removes a provider and returns `true` if it existed.

```lua
uji.provider.remove("perplexity")
```

## uji.provider.list()

Returns one table per provider with `id`, `name`, `api`, `base_url`,
`auth_env`, `context_window`, `models`, `oauth`, which is `true` when the
provider offers subscription sign-in, and `state`. `state` is `"loaded"` once
the models are in, `"idle"` before uji has called a `models` function,
`"loading"` while it runs, and `"failed"` when it raised an error. Each model
has `id`, `context`, `output`, `reasoning`, `cache`, `images` and `efforts`.

```lua
for _, provider in ipairs(uji.provider.list()) do
  if provider.base_url:find("localhost", 1, true) then
    uji.notify(provider.name)
  end
end
```

## uji.provider.models(id, on_done)

Loads the provider's models if they are not loaded yet, waits for them, and
returns them in the format `uji.provider.list()` uses. When loading fails, it
returns the models the provider already has and the error message.
With `on_done`, it returns at once and calls `on_done` with the same values.

```lua
local models, err = uji.provider.models("ollama")
if err then
  uji.notify("could not list Ollama models: " .. err)
end
```
