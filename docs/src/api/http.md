# uji.http

## uji.http.request(opts, on_done)

Sends an HTTP request and returns a function that cancels it. `on_done`
receives a response table with `status`, `headers` and `body`, or `nil` and an
error message. Header names are lower case.

| Option | Type | Meaning |
|---|---|---|
| `url` | string | Required. The URL. |
| `method` | string | The HTTP method. The default is `"GET"`. |
| `headers` | table | Request headers, name to value. |
| `body` | string | The request body. |
| `timeout` | number | Seconds before the request fails. The default is 30 for a plain request and none for a streamed one. |
| `on_line` | function | Streams the response. uji calls it with each line of the body as it arrives. Return `false` for a line that is not progress, such as a keep-alive. |
| `idle` | number | For a streamed request, the seconds without progress before uji gives up. The default is 120. |

A non-2xx status still counts as a response. Check `status` yourself. An
invalid URL, method or header raises an error.

```lua
uji.http.request({
  url = "https://api.github.com/repos/uji-labs/uji",
  headers = { Accept = "application/vnd.github+json" },
}, function(response, err)
  if not response then
    uji.notify("request failed: " .. err)
  elseif response.status == 200 then
    uji.notify(uji.json.decode(response.body).description)
  end
end)
```
