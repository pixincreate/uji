use std::collections::BTreeSet;
use std::path::PathBuf;

use mlua::{Function, Lua, MultiValue, Table, Value};
use uji_macros::function;

use crate::kernel::State;

const AFTER_PRELOAD: i64 = 2;

#[derive(Clone)]
pub enum Sources {
    Embedded(&'static [(&'static str, &'static str)]),
    Directory(PathBuf),
}

impl Sources {
    fn find(&self, module: &str) -> Option<(String, Vec<u8>)> {
        let relative = module.replace('.', "/");
        [format!("{relative}.lua"), format!("{relative}/init.lua")]
            .into_iter()
            .find_map(|file| self.read(&file).map(|source| (self.name(&file), source)))
    }

    fn read(&self, file: &str) -> Option<Vec<u8>> {
        match self {
            Self::Embedded(files) => files
                .iter()
                .find(|(name, _)| *name == file)
                .map(|(_, source)| source.as_bytes().to_vec()),
            Self::Directory(root) => std::fs::read(root.join(file)).ok(),
        }
    }

    fn children(&self, namespace: &str) -> Vec<String> {
        let dir = namespace.replace('.', "/");
        match self {
            Self::Embedded(files) => files
                .iter()
                .filter_map(|(file, _)| file.strip_prefix(dir.as_str())?.strip_prefix('/'))
                .filter_map(child)
                .collect(),
            Self::Directory(root) => std::fs::read_dir(root.join(&dir))
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let name = entry.file_name().into_string().ok()?;
                    if entry.path().join("init.lua").is_file() {
                        Some(name)
                    } else {
                        child(&name)
                    }
                })
                .collect(),
        }
    }

    fn name(&self, file: &str) -> String {
        match self {
            Self::Embedded(_) => file.to_string(),
            Self::Directory(root) => root.join(file).display().to_string(),
        }
    }
}

fn child(rest: &str) -> Option<String> {
    let name = rest
        .strip_suffix("/init.lua")
        .or_else(|| rest.strip_suffix(".lua"))?;
    (name != "init" && !name.contains('/')).then(|| name.to_string())
}

pub(crate) fn create(layers: Vec<Sources>) -> mlua::Result<Lua> {
    let lua = Lua::new();
    install_searcher(&lua, layers)?;
    let uji = lua.create_table()?;
    for register in crate::REGISTERED {
        register(&lua, &uji)?;
    }
    lua.globals().set("uji", uji)?;
    Ok(lua)
}

pub(crate) fn table(lua: &Lua, root: &Table, path: &[&str]) -> mlua::Result<Table> {
    let mut table = root.clone();
    for name in path {
        table = if let Some(inner) = table.raw_get::<Option<Table>>(*name)? {
            inner
        } else {
            let inner = lua.create_table()?;
            table.raw_set(*name, &inner)?;
            inner
        };
    }
    Ok(table)
}

#[function]
fn modules(lua: &Lua, namespace: &str) -> mlua::Result<Vec<String>> {
    let names: BTreeSet<String> = State::of(lua)?
        .layers
        .iter()
        .flat_map(|layer| layer.children(namespace))
        .collect();
    Ok(names
        .into_iter()
        .map(|name| format!("{namespace}.{name}"))
        .collect())
}

pub(crate) fn entry(lua: &Lua, module: &str) -> mlua::Result<Function> {
    lua.globals().get::<Function>("require")?.call(module)
}

fn install_searcher(lua: &Lua, layers: Vec<Sources>) -> mlua::Result<()> {
    let searcher = lua.create_function(move |lua, module: String| {
        let Some((name, source)) = layers.iter().find_map(|layer| layer.find(&module)) else {
            return Ok(MultiValue::from_vec(vec![Value::String(
                lua.create_string(format!("\n\tno runtime module '{module}'"))?,
            )]));
        };
        let chunk = lua
            .load(source)
            .set_name(format!("@{name}"))
            .into_function()?;
        Ok(MultiValue::from_vec(vec![
            Value::Function(chunk),
            Value::String(lua.create_string(&name)?),
        ]))
    })?;
    let package: Table = lua.globals().get("package")?;
    let searchers: Table = package.get("searchers")?;
    searchers.raw_insert(AFTER_PRELOAD, searcher)
}
