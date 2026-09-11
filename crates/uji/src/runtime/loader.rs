use std::fmt::Write as _;
use std::path::PathBuf;
use std::rc::Rc;

use mlua::{Function, Integer, Lua, MultiValue, Table, Value};
use uji_api::Api;

use uji_core::config::MODULE_DIR;

const INSERT_AFTER_PRELOAD: Integer = 2;

fn candidates(api: &Api, module: &str) -> Vec<PathBuf> {
    let relative = module.replace('.', "/");
    let mut out = Vec::new();
    for root in api.packs().borrow().iter() {
        let base = root.join(MODULE_DIR);
        out.push(base.join(format!("{relative}.lua")));
        out.push(base.join(&relative).join("init.lua"));
    }
    out
}

fn searcher(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(move |lua, module: String| {
        let mut tried = String::new();
        for path in candidates(&api, &module) {
            if !path.is_file() {
                let _ = write!(tried, "\n\tno file '{}'", path.display());
                continue;
            }
            let source = std::fs::read(&path).map_err(|err| {
                mlua::Error::runtime(format!("cannot read {}: {err}", path.display()))
            })?;
            let name = path.display().to_string();
            let chunk = lua.load(&source).set_name(&name).into_function()?;
            return Ok(MultiValue::from_iter([
                Value::Function(chunk),
                Value::String(lua.create_string(&name)?),
            ]));
        }
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
