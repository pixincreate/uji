use std::borrow::Cow;
use std::collections::BTreeSet;
use std::path::PathBuf;

use mlua::{FromLua, Lua, LuaOptions, MultiValue, StdLib, Table, Value};
use uji_native::{Json, native};

use crate::context;

const AFTER_PRELOAD: i64 = 2;

#[derive(Clone)]
pub enum Sources {
    Embedded(&'static [(&'static str, &'static str)]),
    Directory(PathBuf),
}

impl Sources {
    fn find(&self, files: &[String]) -> Option<(String, Cow<'_, [u8]>)> {
        files
            .iter()
            .find_map(|file| self.read(file).map(|source| (self.name(file), source)))
    }

    fn read(&self, file: &str) -> Option<Cow<'_, [u8]>> {
        match self {
            Self::Embedded(files) => files
                .iter()
                .find(|(name, _)| *name == file)
                .map(|(_, source)| Cow::Borrowed(source.as_bytes())),
            Self::Directory(root) => std::fs::read(root.join(file)).ok().map(Cow::Owned),
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

fn candidates(module: &str) -> [String; 2] {
    let relative = module.replace('.', "/");
    [format!("{relative}.lua"), format!("{relative}/init.lua")]
}

fn child(rest: &str) -> Option<String> {
    let name = rest
        .strip_suffix("/init.lua")
        .or_else(|| rest.strip_suffix(".lua"))?;
    (name != "init" && !name.contains('/')).then(|| name.to_string())
}

#[allow(unsafe_code)]
fn lua() -> Lua {
    unsafe { Lua::unsafe_new_with(StdLib::ALL_SAFE | StdLib::FFI, LuaOptions::new()) }
}

pub(crate) fn create(layers: Vec<Sources>) -> mlua::Result<Lua> {
    let lua = lua();
    install_searcher(&lua, layers)?;
    lua.globals().set("uji", lua.create_table()?)?;
    crate::native::install(&lua)?;
    Ok(lua)
}

#[native]
fn modules(namespace: &str) -> Json<Vec<String>> {
    let names: BTreeSet<String> = context::with(|context| {
        context
            .layers
            .iter()
            .flat_map(|layer| layer.children(namespace))
            .collect()
    })
    .unwrap_or_default();
    Json(
        names
            .into_iter()
            .map(|name| format!("{namespace}.{name}"))
            .collect(),
    )
}

pub(crate) fn require<T: FromLua>(lua: &Lua, module: &str) -> mlua::Result<T> {
    lua.globals().get::<mlua::Function>("require")?.call(module)
}

fn install_searcher(lua: &Lua, layers: Vec<Sources>) -> mlua::Result<()> {
    let searcher = lua.create_function(move |lua, module: String| {
        let files = candidates(&module);
        let Some((name, source)) = layers.iter().find_map(|layer| layer.find(&files)) else {
            return Ok(MultiValue::from_vec(vec![Value::String(
                lua.create_string(format!("\n\tno runtime module '{module}'"))?,
            )]));
        };
        let chunk = lua
            .load(source.as_ref())
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
