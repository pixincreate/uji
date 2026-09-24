# An approval rule

## Rules in the policy

Read-only git commands and the tests run without asking, and force-pushes are
refused.

```lua
uji.tool.policy({
  run_command = {
    allow = { "git status", "git diff*", "git log*", "cargo test*" },
    deny = { "/git push.*(--force|-f)/" },
  },
})
```

## A hook for decisions that need code

Refuses `git push`, and asks before commands that reach the network.

```lua
uji.on("before_tool", function(call)
  if call.name ~= "run_command" then
    return nil
  end
  local command = call.arguments.command or ""
  if command:match("^git push") then
    return { deny = "Pushing is my job. Tell me when the branch is ready." }
  end
  if command:match("curl") or command:match("wget") then
    return { ask = "This command reaches the network. Run it?" }
  end
end, { priority = 10 })
```
