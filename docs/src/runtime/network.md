# Network

Each call here waits inside the current [task](tasks.md), so the rest of uji
keeps drawing and taking keys meanwhile.

## uji.net.request(opts)

Sends an HTTP request and waits for the answer, a table with `status`,
`headers` and `body`. When no answer comes back the result is `nil` and an
error message. An error status still counts as an answer.

| Option | Type | Meaning |
|---|---|---|
| `url` | string | The address. Required. |
| `method` | string | The method. The default is `"GET"`. |
| `headers` | table | Header names and values. |
| `body` | string | The request body. |
| `timeout` | number | Seconds to wait for the whole answer. |

## uji.net.open(opts)

Sends a request like `uji.net.request`, but hands back a response object as
soon as the headers arrive, so the body can be read while it streams. It takes
the same options plus `idle`, the seconds without data after which reading
fails, 120 by default. A failed request gives `nil` and an error message.

| Member | Meaning |
|---|---|
| `response.status` | The status code. |
| `response.headers` | Header names and values. |
| `response:line(seconds)` | The next line, `nil` at the end of the body, or `false` if `seconds` pass first. Without `seconds` it waits as long as the line takes. |
| `response:lines()` | An iterator over the remaining lines, for a `for` loop. |
| `response:read()` | Everything left in the body, once it has arrived. |

## uji.net.listen(port)

Listens on `port` on the local machine, or on a free port when `port` is `0`.
The result is a server, or `nil` and an error message.

| Member | Meaning |
|---|---|
| `server.port` | The port it listens on. |
| `server:accept()` | The next connection, or `nil` and an error message once the server is closed. |
| `server:close()` | Stops listening. |
| `conn:line(seconds)` | The next line, `nil` when the other side closes, or `false` if `seconds` pass first. Without `seconds` it waits as long as the line takes. |
| `conn:read(count)` | Exactly `count` bytes. |
| `conn:write(data)` | Sends `data`. |
| `conn:close()` | Closes the connection. |
