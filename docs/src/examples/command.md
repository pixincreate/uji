# A slash command

`/switch` lists your git branches in a picker and switches to the one you
choose. Alt+G opens it.

```lua
uji.command.add("switch", {
  desc = "switch git branch",
  handler = function()
    local branches = {}
    uji.job.start({
      cmd = { "git", "branch", "--format=%(refname:short)" },
      on_stdout = function(line)
        branches[#branches + 1] = line
      end,
      on_exit = function()
        uji.ui.select({ title = "Switch to", items = branches }, function(choice)
          if not choice then
            return
          end
          uji.job.start({
            cmd = { "git", "switch", choice },
            on_exit = function(code)
              uji.notify(code == 0 and ("on " .. choice) or ("git switch failed with " .. code))
              uji.emit("status_changed", {})
            end,
          })
        end)
      end,
    })
  end,
})

uji.keymap.add("normal", "<A-g>", { command = "switch" })
```

The handler receives the text typed after the command name.

```lua
uji.command.add("ask", function(args)
  uji.session.submit("Investigate before changing anything. " .. args)
end)
```
