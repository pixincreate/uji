//! Built-in `uji.*` function implementations.

use std::rc::Rc;

use mlua::{Function, Lua as LuaState, Table, Value as LuaValue};
use tui::model::{Border, BufferKind, Size, Split, WinOpts};

use super::Lua;
use crate::runtime::Inner;

/// `uji.create_buf(name [, { kind = "messages" | "input" }]) -> name`
pub(crate) struct CreateBuf {
    inner: Rc<Inner>,
}

impl Lua for CreateBuf {
    fn key(&self) -> &'static str {
        "create_buf"
    }

    fn name(&self) -> &'static str {
        "uji.create_buf"
    }

    fn create_function(&self, lua: &LuaState) -> mlua::Result<Function> {
        let state = self.inner.state.clone();
        lua.create_function(move |_, (name, opts): (String, Option<Table>)| {
            let kind = opts
                .and_then(|t| t.get::<Option<String>>("kind").ok().flatten())
                .unwrap_or_else(|| name.clone());
            let kind = parse_kind(&kind)?;
            state.borrow_mut().push_buffer(&name, kind);
            Ok(name)
        })
    }
}

/// `uji.open_win(buf [, { split = …, size = …, border = …, title = … }])`
///
/// Buffers referenced by name are auto-created when the name matches a
/// built-in kind.
pub(crate) struct OpenWin {
    inner: Rc<Inner>,
}

impl Lua for OpenWin {
    fn key(&self) -> &'static str {
        "open_win"
    }

    fn name(&self) -> &'static str {
        "uji.open_win"
    }

    fn create_function(&self, lua: &LuaState) -> mlua::Result<Function> {
        let state = self.inner.state.clone();
        lua.create_function(move |_, (buf, opts): (String, Option<Table>)| {
            {
                let mut state = state.borrow_mut();
                if !state.has_buffer(&buf) {
                    let kind = parse_kind(&buf)?;
                    state.push_buffer(&buf, kind);
                }
            }
            let opts = parse_win_opts(opts)?;
            state.borrow_mut().push_window(buf, opts);
            Ok(())
        })
    }
}

/// `uji.schedule(fn)` — run `fn` at the next safe point on the loop
/// (nvim's `vim.schedule`).
pub(crate) struct Schedule {
    inner: Rc<Inner>,
}

impl Lua for Schedule {
    fn key(&self) -> &'static str {
        "schedule"
    }

    fn name(&self) -> &'static str {
        "uji.schedule"
    }

    fn create_function(&self, lua: &LuaState) -> mlua::Result<Function> {
        let inner = self.inner.clone();
        lua.create_function(move |_, callback: Function| {
            inner.scheduled.push(callback);
            Ok(())
        })
    }
}

/// `uji.on(event, fn)` — register a handler; `fn(name, ctx)` is called when
/// `event` is emitted.
pub(crate) struct On {
    inner: Rc<Inner>,
}

impl Lua for On {
    fn key(&self) -> &'static str {
        "on"
    }

    fn name(&self) -> &'static str {
        "uji.on"
    }

    fn create_function(&self, lua: &LuaState) -> mlua::Result<Function> {
        let inner = self.inner.clone();
        lua.create_function(move |_, (event, handler): (String, Function)| {
            inner.handlers.borrow_mut().add(event, handler);
            Ok(())
        })
    }
}

/// `uji.emit(event, ctx)` — dispatch an event to registered handlers.
pub(crate) struct Emit {
    inner: Rc<Inner>,
}

impl Lua for Emit {
    fn key(&self) -> &'static str {
        "emit"
    }

    fn name(&self) -> &'static str {
        "uji.emit"
    }

    fn create_function(&self, lua: &LuaState) -> mlua::Result<Function> {
        let inner = self.inner.clone();
        lua.create_function(move |_, (event, ctx): (String, Table)| {
            inner.dispatch(&event, &ctx);
            Ok(())
        })
    }
}

/// `uji.notify(msg)` — sanctioned stderr channel for config/plugins.
pub(crate) struct Notify;

impl Lua for Notify {
    fn key(&self) -> &'static str {
        "notify"
    }

    fn name(&self) -> &'static str {
        "uji.notify"
    }

    fn create_function(&self, lua: &LuaState) -> mlua::Result<Function> {
        lua.create_function(move |_, message: String| {
            // ast-grep-ignore: no-print-in-lib
            eprintln!("uji: {message}");
            Ok(())
        })
    }
}

/// Build the `uji` API table with all built-in functions registered.
pub(crate) fn api_table(lua: &LuaState, inner: &Rc<Inner>) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    let functions: Vec<Box<dyn Lua>> = vec![
        Box::new(CreateBuf {
            inner: inner.clone(),
        }),
        Box::new(OpenWin {
            inner: inner.clone(),
        }),
        Box::new(Schedule {
            inner: inner.clone(),
        }),
        Box::new(On {
            inner: inner.clone(),
        }),
        Box::new(Emit {
            inner: inner.clone(),
        }),
        Box::new(Notify),
    ];
    for function in &functions {
        function.register(lua, &table)?;
    }
    Ok(table)
}

fn parse_kind(kind: &str) -> Result<BufferKind, mlua::Error> {
    match kind {
        "messages" => Ok(BufferKind::Messages),
        "input" => Ok(BufferKind::Input),
        other => Err(mlua::Error::runtime(format!(
            "unknown buffer kind: {other} (expected \"messages\" or \"input\")"
        ))),
    }
}

fn parse_win_opts(opts: Option<Table>) -> Result<WinOpts, mlua::Error> {
    let mut out = WinOpts::default();
    let Some(opts) = opts else {
        return Ok(out);
    };

    if let Some(split) = opts.get::<Option<String>>("split")? {
        out.split = match split.as_str() {
            "top" => Split::Top,
            "bottom" => Split::Bottom,
            "left" => Split::Left,
            "right" => Split::Right,
            other => return Err(mlua::Error::runtime(format!("unknown split: {other}"))),
        };
    }

    if let Some(size) = opts.get::<Option<LuaValue>>("size")? {
        out.size = match size {
            LuaValue::Integer(n) => match u16::try_from(n) {
                Ok(n) => Size::Fixed(n),
                Err(_) => return Err(mlua::Error::runtime("size must fit in u16")),
            },
            LuaValue::String(s) if s.to_str()? == "fill" => Size::Fill,
            _ => return Err(mlua::Error::runtime("size must be a number or \"fill\"")),
        };
    }

    if let Some(border) = opts.get::<Option<String>>("border")? {
        out.border = match border.as_str() {
            "none" => Border::None,
            "plain" => Border::Plain,
            "rounded" => Border::Rounded,
            other => return Err(mlua::Error::runtime(format!("unknown border: {other}"))),
        };
    }

    out.title = opts.get::<Option<String>>("title")?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Inner;

    #[test]
    fn api_table_registers_the_builtin_keys() {
        let inner = Inner::new(LuaState::new());
        // ast-grep-ignore: no-expect-in-lib
        let table = api_table(&inner.lua, &inner).expect("api table builds");
        for key in ["create_buf", "open_win", "schedule", "on", "emit", "notify"] {
            assert!(table.get::<Function>(key).is_ok(), "missing {key}");
        }
    }

    #[test]
    fn keys_and_names_are_exposed() {
        let inner = Inner::new(LuaState::new());
        let create = CreateBuf {
            inner: inner.clone(),
        };
        assert_eq!(create.key(), "create_buf");
        assert_eq!(create.name(), "uji.create_buf");

        let schedule = Schedule { inner };
        assert_eq!(schedule.key(), "schedule");
        assert_eq!(schedule.name(), "uji.schedule");
    }
}
