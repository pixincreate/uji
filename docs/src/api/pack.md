# uji.pack

### uji.pack.add(specs)

Installs and loads packs. `specs` is a list, and each entry is one of these:
- a `"user/repo"` GitHub shorthand or a git URL
- a table with the URL or shorthand first, and one of `tag`, `branch` or
  `commit`
- a table with `url` instead of the first entry
- a table with `dir`, a local directory that uji never clones

A table may also set `name`, which defaults to the repository or directory
name. uji reports a pack it cannot install as a notice and keeps going.
Raises an error when `specs` is not a list.

```lua
uji.pack.add({
  { dir = "~/code/my-plugin" },
})
```

### uji.pack.list()

Returns every directory uji searches for modules and `plugin/` files, starting
with your config directory.

```lua
for _, root in ipairs(uji.pack.list()) do
  uji.notify(root)
end
```

### uji.pack.update()

Pulls every installed git pack and records the new commits in the lock file.
`/sync` does the same and then reloads.

```lua
uji.pack.update()
```
