# Timers and notices

Both timers run their function as a [task](../runtime/tasks.md), so it can
wait.

## uji.schedule(callback)

Runs `callback` once the code that called it has finished. Called from your
config, it runs after uji has loaded the whole config and every plugin.

```lua
uji.schedule(function()
  uji.notify("config loaded")
end)
```

## uji.defer(seconds, callback)

Runs `callback` after a delay and returns a function that cancels it.

```lua
local cancel = uji.defer(30, function()
  uji.notify("thirty seconds passed")
end)
```

## uji.notify(message)

Shows a notice in the transcript.

```lua
uji.notify("hello from init.lua")
```
