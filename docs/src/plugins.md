# Plugins

The [uji-plugins](https://github.com/uji-labs/uji-plugins) pack holds
optional features. Install it once, then call `setup` for each plugin you
want.

```lua
uji.pack.add({ "uji-labs/uji-plugins" })
```

## statusline

A footer with the directory, model, reasoning effort, context use, tokens,
cache hit rate and turns.

```lua
require("statusline").setup({ separator = " | " })
```

| Option | Meaning | Default |
|---|---|---|
| `defaults` | `false` registers none of the built-in segments. | `true` |
| `separator` | Text between segments. | `"  ·  "` |
| `priority` | The window's layout priority. | `10` |
| `split` | Where the window opens. | `"bottom"` |
| `win` | An existing window to draw into. | none |

Add a segment of your own with [`uji.status.add`](api/status.md#ujistatusaddname-render-opts).

## planmode

A read-only mode. The model investigates and writes a plan, then you accept
it, keep planning, or leave.

```lua
require("planmode").setup({ allow = { "npm test" } })
```

| Option | Meaning | Default |
|---|---|---|
| `allow` | Command prefixes that run without asking while planning. | none |
| `confirm` | `false` skips the accept picker at the end of a turn. | `true` |
| `keys` | `false` leaves Ctrl+B unbound. | `true` |
| `priority` | The priority of its `before_tool` hook. | `10` |

Commands are `/plan`, `/plan <task>` and `/approve`. Ctrl+B toggles plan mode.

## mcp

Tools from MCP servers, over stdio or HTTP.

```lua
require("mcp").setup({
  servers = {
    files = { cmd = { "npx", "-y", "@modelcontextprotocol/server-filesystem", "." } },
    linear = { url = "https://mcp.linear.app/mcp" },
  },
})
```

A server has `cmd` and optional `cwd`, or `url` and optional `token`. Its
tools are named `server__tool`, and each call asks you first unless your
[policy](api/tool.md#ujitoolpolicyrules) allows it. `/mcp add URL` connects a
server and signs you in when it asks, `/mcp remove` disconnects one, and
`/mcp` lists them.

## telescope

A fuzzy finder.

```lua
require("telescope").setup({})
```

| Key | Command | Does |
|---|---|---|
| Ctrl+P | `/find` | Opens a file in your editor. |
| Ctrl+A | `/attach` | Adds `@path` to the input line. |
| Ctrl+G | `/branch` | Asks the model about a git branch. |
| Ctrl+R | `/history` | Refills the input with a past message. |
| Ctrl+F | | Searches file contents as you type. |
| | `/grep <pattern>` | Opens a matching file. |

`editor` sets the program that opens files. It defaults to `UJI_EDITOR`, then
`VISUAL`, then `EDITOR`. `keys = false` leaves the keys unbound.

## skills

Announces Agent Skills folders to the model.

```lua
require("skills").setup({ roots = { "~/.agents/skills", ".agents/skills" } })
```

`roots` defaults to `~/.agents/skills`, `.uji/skills` and `.agents/skills`.
`/skills` lists what it found.

## websearch

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

## readonly

Refuses edits in the directories you list, so the model can read them but not
change them.

```lua
uji.tool.roots({ "~/reference/other-project" })
require("readonly").setup({ "~/reference/other-project" })
```
