# websearch

A `web_search` tool.

```lua
require("websearch").setup({ count = 8 })
```

| Option | Meaning | Default |
|---|---|---|
| `backend` | `"brave"`, `"duckduckgo"` or `"auto"`, which uses Brave when its key is set. | `"auto"` |
| `key_env` | The variable holding the Brave API key. | `"BRAVE_API_KEY"` |
| `count` | Results per search. | `5` |
| `timeout` | Seconds per request. | `20` |
