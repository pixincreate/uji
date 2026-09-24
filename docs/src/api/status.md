# uji.status

`uji.status` reads what uji is doing, and keeps the list of footer segments
that a status line plugin draws.

### uji.status.provider()

Returns the name of the current provider, or `nil` before one is set.

```lua
local provider = uji.status.provider()
```

### uji.status.model()

Returns the current model id, or `nil` before one is set.

```lua
local model = uji.status.model()
```

### uji.status.effort()

Returns the reasoning effort, such as `"medium"`, or `nil` when reasoning is
off.

```lua
local effort = uji.status.effort() or "off"
```

### uji.status.context()

Returns a table with `used`, the estimated tokens in the conversation, and
`window`, the model's context size when uji knows it.

```lua
local context = uji.status.context()
if context.window then
  uji.notify(string.format("%d%% of context used", context.used * 100 // context.window))
end
```

### uji.status.queue()

Returns the messages you typed while the model worked, which uji has not sent
yet.

```lua
local waiting = #uji.status.queue()
```

### uji.status.state()

Returns `"working"` while a turn runs and `"idle"` otherwise.

```lua
local busy = uji.status.state() == "working"
```

### uji.status.elapsed()

Returns the seconds since the current turn started, or `nil` when idle.

```lua
local seconds = math.floor(uji.status.elapsed() or 0)
```

### uji.status.loader_frame()

Returns the loader frame to draw now, from `waiting.loader.frames` in
[`uji.ui.configure`](ui.md#ujiuiconfigureopts). It returns an empty
string when idle.

```lua
local frame = uji.status.loader_frame()
```

### uji.status.add(name, render, opts)

Registers a footer segment. `render` returns any value the status line plugin
understands, or `nil` to hide the segment. `opts.priority` orders segments,
lowest first. The default is 50.

```lua
uji.status.add("model", function()
  return { text = uji.status.model() or "no model", color = "cyan" }
end, { priority = 10 })
```

### uji.status.remove(name)

Removes a segment and returns `true` if it existed.

```lua
uji.status.remove("model")
```

### uji.status.list()

Returns the segment names in priority order.

```lua
local segments = uji.status.list()
```

### uji.status.render()

Calls every segment in priority order and returns the values that are not
`nil`. A status line plugin calls it to draw the footer.

```lua
local parts = uji.status.render()
```
