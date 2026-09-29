# uji.auth

## uji.auth.configure(opts)

Chooses where uji keeps API keys and subscription sign-ins.

| Field | Type | Meaning |
|---|---|---|
| `keychain` | boolean | With `true`, uji saves credentials in the system keychain and looks there first. The default is `false`. |

Without the keychain, uji keeps credentials in `auth.toml` in the
[data directory](../configuration/files.md), readable only by you. Each
provider gets a table named by its id, and a provider can also be a plain
string, which uji reads as an API key. A keychain that refuses a save sends
the credential to `auth.toml` as well.

Raises an error for an unknown field, and for a `keychain` that is not a
boolean.
