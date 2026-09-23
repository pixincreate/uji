use std::rc::Rc;

use mlua::{Function, Lua, Table};
use strum::IntoStaticStr;

use crate::api::Api;
use crate::api::bind::bind;
use crate::api::request::Request;

pub enum ModalKind {
    Select { items: Vec<String> },
    Pick { items: Vec<String>, live: bool },
    Prompt { value: String, hidden: bool },
}

pub struct ModalRequest {
    pub title: String,
    pub kind: ModalKind,
    pub on_done: Function,
}

pub enum Answer {
    Select(Option<String>),
    Prompt(Option<String>),
}

pub fn select(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(
        lua,
        api,
        move |api, _, (opts, on_done): (Table, Function)| {
            let items = opts
                .get::<Option<Vec<String>>>("items")?
                .unwrap_or_default();
            api.request(Request::Modal(Box::new(ModalRequest {
                title: title_of(&opts)?,
                kind: ModalKind::Select { items },
                on_done,
            })));
            Ok(())
        },
    )
}

/// A picker: fuzzy-filtered results with a preview of the highlighted one.
///
/// `preview` overrides the built-in `path:line:` previewer; `on_query` makes the
/// item list live, re-fetched per keystroke.
pub fn pick(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(
        lua,
        api,
        move |api, _, (opts, on_done): (Table, Function)| {
            let items = opts
                .get::<Option<Vec<String>>>("items")?
                .unwrap_or_default();
            let preview = opts.get::<Option<Function>>("preview")?;
            let on_query = opts.get::<Option<Function>>("on_query")?;
            api.pick().borrow_mut().open(preview, on_query.clone());
            api.request(Request::Modal(Box::new(ModalRequest {
                title: title_of(&opts)?,
                kind: ModalKind::Pick {
                    items,
                    live: on_query.is_some(),
                },
                on_done,
            })));
            Ok(())
        },
    )
}

pub(crate) fn show(lua: &Lua, api: &Rc<Api>, token: u64) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, items: Vec<String>| {
        api.request(Request::PickItems { items, token });
        Ok(())
    })
}

pub fn prompt(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(
        lua,
        api,
        move |api, _, (opts, on_done): (Table, Function)| {
            let value = opts.get::<Option<String>>("value")?.unwrap_or_default();
            let hidden = opts.get::<Option<bool>>("hidden")?.unwrap_or(false);
            api.request(Request::Modal(Box::new(ModalRequest {
                title: title_of(&opts)?,
                kind: ModalKind::Prompt { value, hidden },
                on_done,
            })));
            Ok(())
        },
    )
}

fn title_of(opts: &Table) -> mlua::Result<String> {
    Ok(opts.get::<Option<String>>("title")?.unwrap_or_default())
}

#[derive(Clone, Copy, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum Component {
    Select,
    Prompt,
}

pub fn ask_ui(lua: &Lua, component: Component, opts: Table, on_done: Function) -> mlua::Result<()> {
    let uji: Table = lua.globals().get("uji")?;
    let ui: Table = uji.get("ui")?;
    let name: &'static str = component.into();
    let handler: Function = ui.get(name)?;
    handler.call::<()>((opts, on_done))
}
