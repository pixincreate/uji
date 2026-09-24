# Configuration

uji reads `~/.config/uji`, or the directory in `UJI_CONFIG_DIR`.

```text
~/.config/uji/
  init.lua        runs first
  plugin/*.lua    runs after init.lua, sorted by file name
  lua/            modules for require()
```

`init.lua` replaces the default config. Start it with
`require("uji.defaults")` to keep the default screen.

```lua
require("uji.defaults")

uji.keymap.add("normal", "<C-p>", { command = "models" })
```

## Packs

[`uji.pack.add`](api/pack.md) installs a directory or git repository with its
own `lua/` and `plugin/`. `/sync` updates them.

```lua
uji.pack.add({
  "uji-labs/uji-plugins",
  { "someone/tool", tag = "v1.2" },
  { dir = "~/code/my-plugin" },
})
```

## Replacing a built-in module

A file in `lua/` with the same name as one of these modules replaces it.

| Module | Returns |
|---|---|
| `uji.prompt` | A table whose `system(env)` returns the system prompt. `env` has `directory` and `os`. |
| `uji.tools.read_file`, `uji.tools.edit_file`, `uji.tools.write_file`, `uji.tools.run_command` | A [tool spec](api/tool.md#ujitooladdname-spec). |
| `uji.providers.<id>` | A [provider spec](api/provider.md#ujiprovideraddspec), such as `uji.providers.anthropic`. |
| `uji.wires.openai_chat`, `uji.wires.anthropic`, `uji.wires.gemini` | A [wire spec](api/wire.md#ujiwireaddname-spec). |

For example, `~/.config/uji/lua/uji/prompt.lua`:

```lua
local M = {}

function M.system(env)
  return "You are a careful coding agent. Work in " .. env.directory .. "."
end

return M
```
