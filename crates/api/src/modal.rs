use std::rc::Rc;

use mlua::{Function, Lua, Table};

use crate::Api;

pub enum ModalKind {
    Select { items: Vec<String> },
    Prompt { value: String, hidden: bool },
}

pub struct ModalRequest {
    pub title: String,
    pub kind: ModalKind,
    pub on_done: Function,
}

pub fn select(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(move |_, (opts, on_done): (Table, Function)| {
        let items = opts
            .get::<Option<Vec<String>>>("items")?
            .unwrap_or_default();
        api.queue_modal(ModalRequest {
            title: title_of(&opts)?,
            kind: ModalKind::Select { items },
            on_done,
        });
        Ok(())
    })
}

pub fn prompt(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(move |_, (opts, on_done): (Table, Function)| {
        let value = opts.get::<Option<String>>("value")?.unwrap_or_default();
        let hidden = opts.get::<Option<bool>>("hidden")?.unwrap_or(false);
        api.queue_modal(ModalRequest {
            title: title_of(&opts)?,
            kind: ModalKind::Prompt { value, hidden },
            on_done,
        });
        Ok(())
    })
}

fn title_of(opts: &Table) -> mlua::Result<String> {
    Ok(opts.get::<Option<String>>("title")?.unwrap_or_default())
}
