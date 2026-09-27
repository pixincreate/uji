# planmode

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
