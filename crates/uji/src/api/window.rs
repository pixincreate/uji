use std::rc::Rc;

use mlua::{Function, Lua, LuaSerdeExt, Table, Value as LuaValue};

use uji_ui::config::UiConfig;
use uji_ui::model::{Builtin, Color, Line, Size, Span, Style, WinOpts};

use super::Api;
use super::convert::FromLuaValue;
use crate::api::bind::bind;
use crate::api::request::Request;

pub fn open_win(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let state = api.state();
    lua.create_function(move |_, opts: Option<Table>| {
        let (builtin, win_opts) = match opts {
            Some(table) => {
                let builtin = match table.get::<Option<String>>("view")? {
                    Some(view) => Some(
                        view.parse::<Builtin>()
                            .map_err(|err| mlua::Error::runtime(err.to_string()))?,
                    ),
                    None => None,
                };
                (builtin, WinOpts::from_lua_value(&LuaValue::Table(table))?)
            }
            None => (None, WinOpts::default()),
        };
        let mut state = state.borrow_mut();
        Ok(state.open_window(builtin, win_opts))
    })
}

pub fn close_win(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let state = api.state();
    lua.create_function(move |_, id: u32| {
        let mut state = state.borrow_mut();
        Ok(state.close_window(id))
    })
}

pub fn set_lines(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let state = api.state();
    lua.create_function(move |_, (id, values): (u32, Table)| {
        let lines = values
            .sequence_values::<LuaValue>()
            .map(|value| line_from_lua(value?))
            .collect::<mlua::Result<Vec<_>>>()?;
        state.borrow_mut().set_window_lines(id, lines);
        Ok(())
    })
}

pub fn clear(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let state = api.state();
    lua.create_function(move |_, id: u32| {
        state.borrow_mut().clear_window(id);
        Ok(())
    })
}

pub fn set_size(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let state = api.state();
    lua.create_function(move |_, (id, size): (u32, LuaValue)| {
        let size = Size::from_lua_value(&size)?;
        state.borrow_mut().set_window_size(id, size);
        Ok(())
    })
}

pub fn set_title(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let state = api.state();
    lua.create_function(move |_, (id, title): (u32, Option<String>)| {
        state.borrow_mut().set_window_title(id, title);
        Ok(())
    })
}

pub fn exec(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, opts: Table| {
        let command = match opts.get::<LuaValue>("cmd")? {
            LuaValue::String(text) => vec![
                String::from("sh"),
                String::from("-c"),
                text.to_string_lossy(),
            ],
            LuaValue::Table(list) => list
                .sequence_values::<String>()
                .collect::<mlua::Result<Vec<_>>>()?,
            _ => return Err(mlua::Error::runtime("cmd must be a string or a list")),
        };
        if command.is_empty() {
            return Err(mlua::Error::runtime("cmd must not be empty"));
        }
        api.request(Request::Exec(command));
        Ok(())
    })
}

pub fn configure(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let state = api.state();
    lua.create_function(move |lua, opts: Table| {
        let config: UiConfig = lua.from_value(LuaValue::Table(opts))?;
        state.borrow_mut().apply_config(&config);
        Ok(())
    })
}

fn span_from_lua(value: LuaValue) -> mlua::Result<Span> {
    match value {
        LuaValue::String(s) => Ok(Span::new(s.to_str()?.to_owned())),
        LuaValue::Table(table) => {
            let text = table.get::<String>("text")?;
            let style = Style {
                fg: parse_color(table.get::<Option<String>>("color")?)?,
                bg: parse_color(table.get::<Option<String>>("bg")?)?,
                bold: table.get::<Option<bool>>("bold")?.unwrap_or(false),
                italic: table.get::<Option<bool>>("italic")?.unwrap_or(false),
                underline: table.get::<Option<bool>>("underline")?.unwrap_or(false),
            };
            Ok(Span::styled(text, style))
        }
        other => Err(mlua::Error::runtime(format!(
            "span must be a string or {{ text = .., color = .. }}, got {other:?}"
        ))),
    }
}

pub fn lines_from_lua(value: LuaValue) -> mlua::Result<Vec<Line>> {
    let LuaValue::Table(table) = value else {
        return Err(mlua::Error::runtime(
            "a renderer must return a list of lines",
        ));
    };
    table
        .sequence_values::<LuaValue>()
        .map(|item| line_from_lua(item?))
        .collect()
}

fn line_from_lua(value: LuaValue) -> mlua::Result<Line> {
    match &value {
        LuaValue::Table(table) if !table.contains_key("text")? => {
            let spans = table
                .sequence_values::<LuaValue>()
                .map(|item| span_from_lua(item?))
                .collect::<mlua::Result<Vec<_>>>()?;
            Ok(Line { spans })
        }
        _ => Ok(Line::single(span_from_lua(value)?)),
    }
}

fn parse_color(value: Option<String>) -> mlua::Result<Option<Color>> {
    match value {
        Some(s) => s
            .parse::<Color>()
            .map(Some)
            .map_err(|err: uji_ui::model::ParseError| mlua::Error::runtime(err.to_string())),
        None => Ok(None),
    }
}
