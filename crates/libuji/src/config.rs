//! Lua config loading — nvim-style `uji` API over the live UI state.
//!
//! Resolution order: `$UJI_CONFIG` → `~/.config/uji/init.lua` → the embedded
//! `config/default.lua`. Config errors are reported to stderr but fall back
//! to the default so the harness keeps running (like nvim surviving a bad
//! init.lua).
//!
//! The [`load`] function is the *static* convenience path for embedders who
//! only want a layout snapshot; the full runtime (`libuji::runtime`) keeps the
//! Lua state alive.

use std::path::PathBuf;

use mlua::{Lua, Table};
use tui::model::{GlobalOpts, UiModel};

use crate::runtime::Inner;

/// The embedded default config — single source of truth for the default UI.
pub const DEFAULT_LUA: &str = include_str!("../config/default.lua");

/// Load a static [`UiModel`] snapshot from config, falling back to the
/// embedded default on any error.
pub fn load() -> UiModel {
    let Some(path) = config_path() else {
        return embedded_default();
    };
    match std::fs::read_to_string(&path) {
        Ok(source) => match from_lua(&source, &path.display().to_string()) {
            Ok(model) => model,
            Err(err) => {
                // ast-grep-ignore: no-print-in-lib
                eprintln!("uji: config error in {}: {err}", path.display());
                embedded_default()
            }
        },
        Err(err) => {
            // ast-grep-ignore: no-print-in-lib
            eprintln!("uji: cannot read config {}: {err}", path.display());
            embedded_default()
        }
    }
}

fn embedded_default() -> UiModel {
    from_lua(DEFAULT_LUA, "default.lua").unwrap_or_default()
}

/// Resolve the user init.lua path, if one exists.
pub(crate) fn config_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var("UJI_CONFIG").ok().filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(path));
    }
    let home = std::env::var("HOME").ok()?;
    let path = PathBuf::from(home).join(".config/uji/init.lua");
    path.is_file().then_some(path)
}

/// Resolve the plugin directory (`~/.config/uji/lua/plugins`), if present.
pub(crate) fn plugin_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let path = PathBuf::from(home).join(".config/uji/lua/plugins");
    path.is_dir().then_some(path)
}

/// Paths to watch for live reload: the user config file, or the config dir.
pub(crate) fn watch_paths() -> Vec<PathBuf> {
    if let Some(path) = std::env::var("UJI_CONFIG").ok().filter(|p| !p.is_empty()) {
        let path = PathBuf::from(path);
        return if path.is_file() {
            vec![path]
        } else {
            Vec::new()
        };
    }
    if let Ok(home) = std::env::var("HOME") {
        let dir = PathBuf::from(home).join(".config/uji");
        if dir.is_dir() {
            return vec![dir];
        }
    }
    Vec::new()
}

/// Run a config chunk against a fresh `uji` API table and collect the model.
fn from_lua(source: &str, name: &str) -> Result<UiModel, mlua::Error> {
    let inner = Inner::new(Lua::new());
    let uji = crate::lua::functions::api_table(&inner.lua, &inner)?;
    let opt = inner.lua.create_table()?;
    opt.set("cursor_blink", true)?;
    uji.set("opt", opt)?;
    inner.lua.globals().set("uji", uji)?;
    inner.lua.load(source).set_name(name).exec()?;

    // Read opts back from globals — users may reassign `uji.opt` entirely.
    let cursor_blink = inner
        .lua
        .globals()
        .get::<Table>("uji")
        .and_then(|uji| uji.get::<Table>("opt"))
        .ok()
        .and_then(|opt| opt.get::<Option<bool>>("cursor_blink").ok().flatten())
        .unwrap_or(true);

    let state = inner.state.borrow();
    Ok(UiModel {
        buffers: state.buffers().to_vec(),
        windows: state.windows().to_vec(),
        opts: GlobalOpts { cursor_blink },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_lua_builds_the_standard_layout() {
        // ast-grep-ignore: no-expect-in-lib
        let model = from_lua(DEFAULT_LUA, "default.lua").expect("default config is valid");
        assert_eq!(model.windows.len(), 2);
        assert_eq!(model.windows[0].buffer, "messages");
        assert_eq!(model.windows[0].opts.size, tui::model::Size::Fill);
        assert_eq!(model.windows[1].buffer, "input");
        assert_eq!(model.windows[1].opts.size, tui::model::Size::Fixed(3));
        assert_eq!(model.windows[1].opts.border, tui::model::Border::Plain);
        assert_eq!(model, UiModel::default());
    }

    #[test]
    fn unknown_buffer_kind_is_rejected() {
        let err = from_lua("uji.open_win(\"nope\", {})", "test").expect_err("must fail");
        assert!(err.to_string().contains("unknown buffer kind"));
    }

    #[test]
    fn bad_size_is_rejected() {
        let err = from_lua("uji.open_win(\"input\", { size = \"huge\" })", "test")
            .expect_err("must fail");
        assert!(err.to_string().contains("size"));
    }

    #[test]
    fn cursor_blink_opt_is_read_back() {
        // ast-grep-ignore: no-expect-in-lib
        let model = from_lua("uji.opt.cursor_blink = false", "test").expect("valid config");
        assert!(!model.opts.cursor_blink);
    }
}
