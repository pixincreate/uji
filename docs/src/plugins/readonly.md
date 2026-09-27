# readonly

Refuses edits in the directories you list, so the model can read them but not
change them.

```lua
uji.tool.roots({ "~/reference/other-project" })
require("readonly").setup({ "~/reference/other-project" })
```
