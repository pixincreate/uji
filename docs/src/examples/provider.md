# A provider

## An OpenAI-compatible endpoint

A LiteLLM proxy on your machine, or any server that speaks the OpenAI chat
format.

```lua
uji.provider.add({
  id = "litellm",
  name = "LiteLLM",
  wire = "openai-chat",
  base_url = os.getenv("LITELLM_BASE_URL") or "http://localhost:4000",
  auth_env = { "LITELLM_API_KEY" },
  models = {
    { id = "claude-sonnet-4-5", context = 1000000, output = 64000, reasoning = true },
    { id = "glm-4.6", context = 204800, output = 131072, reasoning = true },
  },
})
```

## Changing a built-in provider

With the id of a built-in provider, `uji.provider.add` changes only the fields
you give.

```lua
uji.provider.add({ id = "openai", base_url = "https://llm-proxy.internal/v1" })

uji.provider.add({
  id = "anthropic",
  models = { { id = "claude-fable-5-1", context = 1000000, output = 128000, reasoning = true } },
})
```
