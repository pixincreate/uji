# System

These describe the machine and the current run, and give access to saved
passwords and to what you last copied.

| Function | Gives |
|---|---|
| `uji.os.platform` | `"macos"`, `"linux"`, `"windows"` or `"other"`. |
| `uji.os.env(name)` | The value of an environment variable, or `nil` when it is unset or empty. |
| `uji.os.cwd()` | The directory uji started in. |
| `uji.os.home()` | Your home directory, or `nil`. |
| `uji.os.now()` | The time in milliseconds since the Unix epoch. |
| `uji.os.clock()` | Seconds since uji started, for measuring how long something took. |
| `uji.os.roots` | The directories whose `lua/` folder is searched before the built-in modules. |
| `uji.os.carry` | The text handed over by the last `uji.os.restart`, or `nil`. |

## uji.os.restart(opts)

Starts uji's Lua side again once the current code yields, keeping the screen.
Every task, connection and process from this run stops. `opts.args` is the
command line for the new run, `opts.roots` sets `uji.os.roots` for it, and
`opts.carry` is text it can read from `uji.os.carry`.

## uji.keychain.get(service, account)

Looks up the secret saved in the system keychain for `service` and `account`,
and gives `nil` when there is none.

## uji.keychain.set(service, account, secret)

Saves `secret` in the system keychain, then gives `true`, or `nil` and an
error message.

## uji.keychain.delete(service, account)

Removes the secret, with the same result as `uji.keychain.set`.

## uji.clipboard.get()

Reads the text on the system clipboard, or gives `nil` and an error message.

## uji.clipboard.set(text)

Puts `text` on the system clipboard and gives `true`, or `nil` and an error
message.
