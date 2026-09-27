# uji.input

These functions read and change what you have typed, and can take over the
keyboard.

## uji.input.get()

Returns the text on the input line.

```lua
local draft = uji.input.get()
```

## uji.input.set(text)

Replaces the text on the input line and puts the cursor at the end. Text such
as `/models` opens the command list, as typing it would.

```lua
uji.input.set("/models")
```

## uji.input.append(text)

Adds text at the end, with the cursor after it.

```lua
uji.input.append(" and add a test")
```

## uji.input.clear()

Empties the input line.

```lua
uji.input.clear()
```

## uji.input.capture(handler)

Sends every key press to `handler` instead of the normal bindings, until
`uji.input.release` runs. The handler receives a table with `key`, such as
`"<C-x>"`, `char` for a printable key, and `ctrl`, `alt` and `shift`. uji
releases the capture if the handler raises an error.

```lua
uji.input.capture(function(event)
  if event.key == "<Esc>" then
    uji.input.release()
  end
end)
```

## uji.input.release()

Returns the keyboard to the normal bindings.

```lua
uji.input.release()
```
