# telescope

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
