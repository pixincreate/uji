use std::rc::Rc;

use mlua::{Lua, Table};
use uji_core::llm::Usage;

use crate::api::Api;
use crate::api::bind::bind;
use crate::api::request::Request;

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let session = lua.create_table()?;

    session.set(
        "info",
        bind(lua, api, move |api, lua, ()| {
            let conversation = api.conversation().borrow();
            let info = conversation.info();
            let out = lua.create_table()?;
            out.set("id", info.id.to_string())?;
            out.set("title", info.title.clone())?;
            out.set("directory", info.directory.clone())?;
            Ok(out)
        })?,
    )?;

    session.set(
        "messages",
        bind(lua, api, move |api, lua, ()| {
            let out = lua.create_table()?;
            let conversation = api.conversation().borrow();
            for stored in conversation.messages() {
                let row = lua.create_table()?;
                row.set("type", stored.message.type_name())?;
                row.set("text", stored.message.text())?;
                if let uji_core::session::model::Message::Tool { name, .. } = &stored.message {
                    row.set("name", name.clone())?;
                }
                out.push(row)?;
            }
            Ok(out)
        })?,
    )?;

    session.set(
        "usage",
        bind(lua, api, move |api, lua, ()| {
            let tally = api.conversation().borrow().tally();
            let out = usage_table(lua, tally.usage)?;
            out.set("last", usage_table(lua, tally.last)?)?;
            out.set("requests", tally.turns)?;
            Ok(out)
        })?,
    )?;

    session.set(
        "set_title",
        bind(lua, api, move |api, _, title: String| {
            let title = title.trim().to_string();
            if title.is_empty() {
                return Err(mlua::Error::runtime("set_title needs non-empty text"));
            }
            api.request(Request::SetTitle(title));
            Ok(())
        })?,
    )?;

    session.set(
        "submit",
        bind(lua, api, move |api, _, text: String| {
            if text.trim().is_empty() {
                return Err(mlua::Error::runtime("submit needs non-empty text"));
            }
            api.request(Request::Submit(text));
            Ok(())
        })?,
    )?;

    session.set(
        "interrupt",
        bind(lua, api, move |api, _, ()| {
            api.request(Request::Interrupt);
            Ok(())
        })?,
    )?;

    Ok(session)
}

fn usage_table(lua: &Lua, usage: Usage) -> mlua::Result<Table> {
    let out = lua.create_table()?;
    out.set("input", usage.input)?;
    out.set("output", usage.output)?;
    out.set("cache_read", usage.cache_read)?;
    out.set("cache_write", usage.cache_write)?;
    out.set("total", usage.total())?;
    Ok(out)
}
