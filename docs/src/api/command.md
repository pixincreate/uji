# uji.command

These functions manage the slash commands that your config and plugins add.
[Slash commands](../getting-started/commands.md#slash-commands) lists the
built-in ones.

## uji.command.add(name, spec)

Registers `/name`, or replaces the Lua command with the same name. `spec` is a
function, or a table with these fields.

| Field | Type | Required | Meaning |
|---|---|---|---|
| `handler` | function | yes | Runs the command. It receives the text typed after the name. |
| `desc` | string | no | The description in the suggestion list. |
| `force` | boolean | no | With `true`, replaces a built-in command with the same name. |

Raises an error when `spec` is neither a function nor a table with a
`handler`.

```lua
uji.command.add("standup", {
  desc = "summarise yesterday's commits",
  handler = function(args)
    uji.session.submit("Summarise the commits since yesterday. " .. args)
  end,
})
```

## uji.command.remove(name)

Removes a Lua command and returns `true` if it existed.

```lua
uji.command.remove("standup")
```

## uji.command.list()

Returns the names of the Lua commands in alphabetical order. Built-in commands
are not in the list.

```lua
local names = uji.command.list()
```
