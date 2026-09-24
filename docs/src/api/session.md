# uji.session

`uji.session` reads the current session and drives it.

### uji.session.info()

Returns a table with the session's `id`, `title` and `directory`.

```lua
local here = uji.session.info().directory
```

### uji.session.messages()

Returns the transcript as a list of tables with `type` and `text`. Tool
results also have `name`.

```lua
local count = 0
for _, message in ipairs(uji.session.messages()) do
  if message.type == "user" then
    count = count + 1
  end
end
```

### uji.session.usage()

Returns the tokens spent in this session. The table has `input`, `output`,
`cache_read`, `cache_write` and `total`. It also has `last`, a table of the
same fields for the latest request, and `requests`, the number of requests.

```lua
local usage = uji.session.usage()
local line = string.format("%d tokens over %d requests", usage.total, usage.requests)
```

### uji.session.set_title(title)

Renames the session, saves the name, and fires `session_titled`. Raises an
error when the title is empty.

```lua
uji.session.set_title("fix the flaky login test")
```

### uji.session.submit(text)

Sends a message as if you typed it. While the model works, uji queues it.
Raises an error when the text is empty.

```lua
uji.session.submit("Run the tests and fix what fails.")
```

### uji.session.interrupt()

Stops the current turn, or the running `!` command.

```lua
uji.session.interrupt()
```
