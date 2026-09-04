//! Lua config loading — nvim-style `uji` API over the tui object model.
//!
//! Resolution order: `$UJI_CONFIG` → `~/.config/uji/init.lua` → the embedded
//! `config/default.lua`. Config errors are reported to stderr but fall back
//! to the default so the harness keeps running (like nvim surviving a bad
//! init.lua).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use mlua::{Lua, Table, Value as LuaValue};
use tui::model::{
    Border, BufferKind, BufferSpec, GlobalOpts, Size, Split, UiModel, WinOpts, WindowSpec,
};

/// The embedded default config — single source of truth for the default UI.
pub const DEFAULT_LUA: &str = include_str!("../config/default.lua");

/// Load the UI model from config, falling back to the embedded default.
pub fn load() -> UiModel {
    let Some(path) = config_path() else {
        return embedded_default();
    };
    match std::fs::read_to_string(&path) {
        Ok(source) => match from_lua(&source, &path.display().to_string()) {
            Ok(model) => model,
            Err(err) => {
                eprintln!("uji: config error in {}: {err}", path.display());
                embedded_default()
            }
        },
        Err(err) => {
            eprintln!("uji: cannot read config {}: {err}", path.display());
            embedded_default()
        }
    }
}

fn embedded_default() -> UiModel {
    from_lua(DEFAULT_LUA, "default.lua").unwrap_or_else(|_| UiModel::default())
}

fn config_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("UJI_CONFIG") {
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    let home = std::env::var("HOME").ok()?;
    let path = PathBuf::from(home).join(".config/uji/init.lua");
    path.is_file().then_some(path)
}

#[derive(Default)]
struct Builder {
    buffers: Vec<BufferSpec>,
    windows: Vec<WindowSpec>,
}

/// Run a config chunk against a fresh `uji` API table and collect the model.
fn from_lua(source: &str, name: &str) -> Result<UiModel, String> {
    let lua = Lua::new();
    let state = Rc::new(RefCell::new(Builder::default()));

    let uji = lua.create_table().map_err(lua_err)?;
    let opt = lua.create_table().map_err(lua_err)?;
    opt.set("cursor_blink", true).map_err(lua_err)?;
    uji.set("opt", opt).map_err(lua_err)?;

    // uji.create_buf(name [, { kind = "messages" | "input" }]) -> name
    let st = state.clone();
    let create_buf = lua
        .create_function(move |_, (name, opts): (String, Option<Table>)| {
            let kind = opts
                .and_then(|t| t.get::<Option<String>>("kind").ok().flatten())
                .unwrap_or_else(|| name.clone());
            let kind = parse_kind(&kind)?;
            st.borrow_mut()
                .buffers
                .push(BufferSpec { name: name.clone(), kind });
            Ok(name)
        })
        .map_err(lua_err)?;
    uji.set("create_buf", create_buf).map_err(lua_err)?;

    // uji.open_win(buf [, { split = …, size = …, border = …, title = … }])
    // Buffers referenced by name are auto-created when the name matches a
    // built-in kind.
    let st = state.clone();
    let open_win = lua
        .create_function(move |_, (buf, opts): (String, Option<Table>)| {
            {
                let mut builder = st.borrow_mut();
                if !builder.buffers.iter().any(|b| b.name == buf) {
                    let kind = parse_kind(&buf)?;
                    builder.buffers.push(BufferSpec { name: buf.clone(), kind });
                }
            }
            let opts = parse_win_opts(opts)?;
            st.borrow_mut().windows.push(WindowSpec { buffer: buf, opts });
            Ok(())
        })
        .map_err(lua_err)?;
    uji.set("open_win", open_win).map_err(lua_err)?;

    lua.globals().set("uji", uji).map_err(lua_err)?;
    lua.load(source)
        .set_name(name)
        .exec()
        .map_err(|err| err.to_string())?;

    // Read opts back from globals — users may reassign `uji.opt` entirely.
    let cursor_blink = lua
        .globals()
        .get::<Table>("uji")
        .and_then(|uji| uji.get::<Table>("opt"))
        .ok()
        .and_then(|opt| opt.get::<Option<bool>>("cursor_blink").ok().flatten())
        .unwrap_or(true);

    let builder = state.borrow();
    Ok(UiModel {
        buffers: builder.buffers.clone(),
        windows: builder.windows.clone(),
        opts: GlobalOpts { cursor_blink },
    })
}

fn lua_err(error: mlua::Error) -> String {
    error.to_string()
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
            LuaValue::Integer(n) if (0..=i64::from(u16::MAX)).contains(&n) => {
                Size::Fixed(n as u16)
            }
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
