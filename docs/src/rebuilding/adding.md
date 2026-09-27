# Adding a module

uji loads every file in the `commands/`, `tools/`, `wires/`, `providers/` and
`api/` folders of `lua/uji/`, and each file registers itself. A new file such
as `~/.config/uji/lua/uji/commands/hello.lua` adds a command without a change
to any other file, and the same works in a pack.

| Folder | What a file there calls |
|---|---|
| `commands/` | `require("uji.command").builtin(name, description, handler)`. The handler receives the text typed after the name. |
| `tools/` | `require("uji.tool").add(name, spec)`, with the fields of [`uji.tool.add`](../api/tool.md#ujitooladdname-spec). |
| `wires/` | `require("uji.wire").add(name, spec)`, with the fields of [`uji.wire.add`](../api/wire.md#ujiwireaddname-spec). |
| `providers/` | `require("uji.catalog").builtin(spec)`, with the fields of [`uji.provider.add`](../api/provider.md#ujiprovideraddspec). |
| `api/` | Nothing. It sets its own table on `uji`, such as `uji.session`. |

Files in a folder load in alphabetical order, so `/login` lists providers and
the command list shows built-in commands in that order.
[`uji.modules`](../runtime/system.md#ujimodulesnamespace) gives the same list
uji loads from.
