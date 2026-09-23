use std::fmt::Write as _;
use std::path::PathBuf;
use std::rc::Rc;

use crate::api::Api;
use mlua::{Function, Integer, Lua, MultiValue, Table, Value};

use uji_agent::config::MODULE_DIR;

use super::bundled;

const INSERT_AFTER_PRELOAD: Integer = 2;

fn files_for(module: &str) -> [String; 2] {
    let relative = module.replace('.', "/");
    [format!("{relative}.lua"), format!("{relative}/init.lua")]
}

fn on_disk(api: &Api, files: &[String]) -> Vec<PathBuf> {
    api.packs()
        .borrow()
        .iter()
        .flat_map(|root| {
            let base = root.join(MODULE_DIR);
            files.iter().map(move |file| base.join(file))
        })
        .collect()
}

fn load(lua: &Lua, source: &[u8], name: &str) -> mlua::Result<MultiValue> {
    let chunk = lua.load(source).set_name(name).into_function()?;
    Ok(MultiValue::from_iter([
        Value::Function(chunk),
        Value::String(lua.create_string(name)?),
    ]))
}

fn searcher(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(move |lua, module: String| {
        let files = files_for(&module);
        let mut tried = String::new();
        for path in on_disk(&api, &files) {
            if !path.is_file() {
                let _ = write!(tried, "\n\tno file '{}'", path.display());
                continue;
            }
            let source = std::fs::read(&path).map_err(|err| {
                mlua::Error::runtime(format!("cannot read {}: {err}", path.display()))
            })?;
            return load(lua, &source, &path.display().to_string());
        }
        if let Some((file, source)) = files
            .iter()
            .find_map(|file| Some((file, bundled::file(file)?)))
        {
            return load(lua, source.as_bytes(), &bundled::name(file));
        }
        let _ = write!(tried, "\n\tno bundled module '{module}'");
        Ok(MultiValue::from_iter([Value::String(
            lua.create_string(&tried)?,
        )]))
    })
}

pub(crate) fn install(lua: &Lua, api: &Rc<Api>) -> mlua::Result<()> {
    let package: Table = lua.globals().get("package")?;
    let searchers: Table = package.get("searchers")?;
    searchers.raw_insert(INSERT_AFTER_PRELOAD, searcher(lua, api)?)?;
    Ok(())
}
