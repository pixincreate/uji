# Native modules

A native module is a shared library written in Rust that Lua loads with
`require`. uji looks for it in `native/` in the
[config directory](../configuration/files.md), where `require("name")` finds
`name.dylib` on macOS, `name.so` on Linux and `name.dll` on Windows.

## The crate

The library is a Rust crate with `crate-type = ["cdylib"]` that depends on
`uji-native`. It calls `uji_native::module!()` once at the top level, which
exports what uji reads when it loads the library.

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
uji-native = { git = "https://github.com/uji-labs/uji" }
```

## #[native]

Every function marked `#[native]` becomes a Lua function on the table that
`require` returns. `#[native(name)]` puts it in a nested table instead, so
`#[native(text)] fn trim` becomes `module.text.trim`.

| Parameter | Lua passes |
|---|---|
| `&str`, `String` | a string |
| `&[u8]`, `Vec<u8>` | a string of bytes |
| `Json<T>` | a table, read into any `T` that implements `Deserialize`. `nil` reads as an empty table. |
| `bool` | any value, where only `true` counts as true |
| integers and floats | a number |
| `Option<number>` | a number or `nil` |
| `Option<&str>` | a string or `nil` |
| `&T` or `Arc<T>` as the first parameter | the object itself, which makes the function a method of `T` |

| Return | Lua receives |
|---|---|
| `bool`, integers and floats | the value |
| `String`, `Vec<u8>` | a string |
| `Json<T>` | a table, for any `T` that implements `Serialize` |
| `()` | `true` |
| `Option<T>` | `nil` for `None` |
| `Result<T, E>` | the value, or `nil` and the error's message |
| `(A, B)` | two values |
| `Held<T, F>` | an object that holds `T`, with the fields of `F` |

A function marked `async` runs on a runtime the library owns. The Lua task
that calls it waits for the result, and other tasks run meanwhile. When that
task is cancelled, the function still runs to its end, and uji drops the
result.
`#[native(raise)]` turns an error from a `Result` into a Lua error instead of
`nil` and a message. An argument of the wrong type always raises an error.

## Objects

`Held::new(value)` returns `value` as an object, and `Held(value, fields)`
also sets the fields that `fields` serializes to on it. A function whose first
parameter is `&T` or `Arc<T>` becomes a method of every object that holds a
`T`. It takes `Arc<T>` when it is `async`, since the object may be released
before the work ends. `#[native(iterate = lines)]` on a method named `line`
also adds `lines()`, which returns an iterator that calls `line` until it
returns `nil`. uji releases the value when Lua no longer holds the object.
