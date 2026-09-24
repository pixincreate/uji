<picture>
  <source media="(prefers-color-scheme: dark)" srcset="images/readme-header-dark.png">
  <img src="images/readme-header-light.png" width="640" alt="uji. A coding agent you can shape with Lua.">
</picture>

# Introduction

uji is a coding agent for your terminal, configured in Lua. This is a
complete `~/.config/uji/init.lua`:

```lua
require("uji.defaults")

uji.pack.add({ "uji-labs/uji-plugins" })
require("statusline").setup({})
require("planmode").setup({})

uji.tool.policy({
  run_command = { allow = { "git status", "cargo test*" } },
})

uji.keymap.add("normal", "<C-p>", { command = "models" })

uji.command.add("standup", function()
  uji.session.submit("Summarise the commits since yesterday.")
end)
```

- [Configuration](configuration.md) shows where your config goes.
- [Available APIs](api/index.md) lists every `uji.*` function.
- [Examples](examples/tool.md) build tools, commands, a footer, providers and approval rules.
- [Plugins](plugins.md) lists the plugins and their options.
