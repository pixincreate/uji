# Encoding

| Function | Gives |
|---|---|
| `uji.base64.encode(data, opts)` | `data` in base64. `opts.url = true` uses the URL alphabet and `opts.pad = false` leaves out padding. |
| `uji.base64.decode(text, opts)` | The decoded bytes. It takes the same options and raises an error for text that is not base64. |
| `uji.sha256(data)` | The SHA-256 digest of `data`, as 32 raw bytes. |
| `uji.random(count)` | `count` random bytes. |
