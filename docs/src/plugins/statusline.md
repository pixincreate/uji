# statusline

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

Add a segment of your own with [`uji.status.add`](../api/status.md#ujistatusaddname-render-opts).
