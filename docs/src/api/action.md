# uji.action

Actions are named functions that keys can run. uji has built-in actions,
listed in [Keys](keymap.md#actions), and you can add your own.

## uji.action.add(name, handler)

Registers an action. A key bound to `name` then runs `handler` with no
arguments. Raises an error when `name` belongs to a built-in action.

```lua
uji.action.add("insert_date", function()
  uji.input.append(os.date("%Y-%m-%d"))
end)
uji.keymap.add("normal", "<A-d>", "insert_date")
```

## uji.action.remove(name)

Removes an action you added and returns `true` if it existed.

```lua
uji.action.remove("insert_date")
```

## uji.action.list()

Returns the names of every action, built-in and added, in alphabetical order.

```lua
local actions = uji.action.list()
```
