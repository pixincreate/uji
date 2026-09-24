# uji.json

### uji.json.encode(value)

Turns a Lua value into a JSON string. An empty table encodes as `{}` unless
[`uji.json.array`](#ujijsonarraytable) marked it.

```lua
local body = uji.json.encode({ model = "gpt-4.1", stream = true })
```

### uji.json.decode(text, opts)

Turns a JSON string into a Lua value. A JSON `null` becomes `uji.json.null`,
so it survives a round trip. With `opts.nulls = false`, `null` becomes `nil`
and the key disappears. Raises an error for invalid JSON.

```lua
local value = uji.json.decode('{"a": 1, "b": null}', { nulls = false })
```

### uji.json.array(table)

Marks a table as a JSON array, so it encodes as `[]` even when empty. Returns
the same table.

```lua
local body = uji.json.encode({ tools = uji.json.array({}) })
```

`uji.json.null` is the value that stands for JSON `null`.
