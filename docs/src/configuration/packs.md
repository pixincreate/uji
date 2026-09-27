# Packs

[`uji.pack.add`](../api/pack.md) installs a directory or git repository with
its own `lua/` and `plugin/`. `/sync` updates them.

```lua
uji.pack.add({
  "uji-labs/uji-plugins",
  { "someone/tool", tag = "v1.2" },
  { dir = "~/code/my-plugin" },
})
```
