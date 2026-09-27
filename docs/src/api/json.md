# uji.json

Plugins use these to build request bodies and to read saved files and HTTP
answers.

## uji.json.encode(value)

Turns a Lua value into a JSON string, where an empty table becomes `{}` unless
[`uji.json.array`](#ujijsonarraytable) marked it.

```lua
local body = uji.json.encode({ model = "gpt-4.1", stream = true })
```

## uji.json.decode(text, opts)

Turns a JSON string into a Lua value. A JSON `null` becomes `uji.json.null`,
so it survives a round trip through `uji.json.encode`, unless you pass
`opts.nulls = false`, which turns it into `nil` and drops the key. Invalid JSON
raises an error.

```lua
local value = uji.json.decode('{"a": 1, "b": null}', { nulls = false })
```

## uji.json.array(table)

Marks a table as a JSON array, so it encodes as `[]` even when empty, and
returns the same table.

```lua
local body = uji.json.encode({ tools = uji.json.array({}) })
```

`uji.json.null` stands for JSON `null`.
