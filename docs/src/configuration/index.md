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
