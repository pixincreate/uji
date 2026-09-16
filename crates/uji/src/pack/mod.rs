pub(crate) mod git;
pub(crate) mod lock;

use std::path::PathBuf;
use std::rc::Rc;

use crate::api::Api;
use mlua::{Function, Lua, Table, Value};

use uji_engine::config;

use self::lock::{Entry, Lock};

const OPTION_KEYS: &[&str] = &["url", "dir", "tag", "branch", "commit", "name"];

enum Source {
    Git {
        url: String,
        reference: Option<String>,
    },
    Dir(PathBuf),
}

struct Spec {
    name: String,
    source: Source,
}

fn repo_name(url: &str) -> String {
    url.trim_end_matches('/')
        .trim_end_matches(".git")
        .rsplit(['/', ':'])
        .next()
        .unwrap_or(url)
        .to_string()
}

fn expand_url(short: &str) -> String {
    let looks_local = short.starts_with('/') || short.starts_with('.') || short.starts_with('~');
    if short.contains("://") || short.starts_with("git@") || looks_local {
        return config::expand_home(short).display().to_string();
    }
    if short.matches('/').count() == 1 {
        return format!("https://github.com/{short}");
    }
    short.to_string()
}

fn spec_from_table(table: &Table) -> Result<Spec, String> {
    let field = |key: &str| table.get::<Option<String>>(key).ok().flatten();

    if let Some(dir) = field("dir") {
        let path = config::expand_home(&dir);
        let name = field("name").unwrap_or_else(|| repo_name(&dir));
        return Ok(Spec {
            name,
            source: Source::Dir(path),
        });
    }

    let short = table.get::<Option<String>>(1).ok().flatten();
    let Some(url) = field("url").or(short) else {
        return Err(String::from(
            "pack spec needs a url, a \"user/repo\" shorthand, or dir",
        ));
    };
    let url = expand_url(&url);
    let reference = field("commit")
        .or_else(|| field("tag"))
        .or_else(|| field("branch"));
    let name = field("name").unwrap_or_else(|| repo_name(&url));
    Ok(Spec {
        name,
        source: Source::Git { url, reference },
    })
}

fn spec_from_value(value: &Value) -> Result<Spec, String> {
    match value {
        Value::String(short) => {
            let short = short.to_string_lossy();
            let url = expand_url(&short);
            Ok(Spec {
                name: repo_name(&url),
                source: Source::Git {
                    url,
                    reference: None,
                },
            })
        }
        Value::Table(table) => spec_from_table(table),
        _ => Err(String::from("pack spec must be a string or a table")),
    }
}

fn is_single_spec(table: &Table) -> bool {
    if table.raw_len() == 0 {
        return true;
    }
    OPTION_KEYS
        .iter()
        .any(|key| matches!(table.get::<Option<Value>>(*key), Ok(Some(_))))
}

fn specs_from_argument(value: &Value) -> Result<Vec<Spec>, String> {
    let Value::Table(table) = value else {
        return spec_from_value(value).map(|spec| vec![spec]);
    };
    if is_single_spec(table) {
        return spec_from_table(table).map(|spec| vec![spec]);
    }
    let mut specs = Vec::new();
    for entry in table.clone().sequence_values::<Value>() {
        let entry = entry.map_err(|err| format!("cannot read pack spec: {err}"))?;
        specs.push(spec_from_value(&entry)?);
    }
    Ok(specs)
}

fn install(spec: &Spec, api: &Api) -> Result<PathBuf, String> {
    match &spec.source {
        Source::Dir(path) => {
            if !path.is_dir() {
                return Err(format!("{}: no such directory", path.display()));
            }
            Ok(path.clone())
        }
        Source::Git { url, reference } => {
            let site = config::site_dir().ok_or_else(|| String::from("$HOME is not set"))?;
            let dir = site.join(&spec.name);
            if dir.is_dir() {
                return Ok(dir);
            }
            let mut locked = Lock::load();
            let pinned = reference
                .clone()
                .or_else(|| locked.get(&spec.name).map(|entry| entry.rev.clone()));
            std::fs::create_dir_all(&site)
                .map_err(|err| format!("mkdir {}: {err}", site.display()))?;
            git::clone(url, &dir, pinned.as_deref())?;
            let rev = git::head(&dir)?;
            locked.set(
                spec.name.clone(),
                Entry {
                    url: url.clone(),
                    rev,
                    reference: reference.clone(),
                },
            );
            locked.save()?;
            api.notify(format!("pack: installed {}", spec.name));
            Ok(dir)
        }
    }
}

pub(crate) fn update_all(api: &Api) {
    let mut locked = Lock::load();
    let Some(site) = config::site_dir() else {
        api.notify(String::from("pack: $HOME is not set"));
        return;
    };
    let entries: Vec<(String, Entry)> = locked
        .entries()
        .map(|(name, entry)| (name.clone(), entry.clone()))
        .collect();
    if entries.is_empty() {
        api.notify(String::from("pack: nothing installed"));
        return;
    }
    let mut changed = false;
    for (name, entry) in entries {
        let dir = site.join(&name);
        if !dir.is_dir() {
            api.notify(format!("pack: {name} is not installed"));
            continue;
        }
        match git::update(&dir, entry.reference.as_deref()) {
            Ok(rev) if rev == entry.rev => api.notify(format!("pack: {name} already current")),
            Ok(rev) => {
                api.notify(format!("pack: updated {name}"));
                locked.set(
                    name,
                    Entry {
                        url: entry.url,
                        rev,
                        reference: entry.reference,
                    },
                );
                changed = true;
            }
            Err(err) => api.notify(format!("pack: {name}: {err}")),
        }
    }
    if changed && let Err(err) = locked.save() {
        api.notify(format!("pack: {err}"));
    }
}

pub(crate) fn add(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(move |_, value: Value| {
        let specs = match specs_from_argument(&value) {
            Ok(specs) => specs,
            Err(message) => return Err(mlua::Error::runtime(message)),
        };
        for spec in specs {
            match install(&spec, &api) {
                Ok(dir) => {
                    let mut roots = api.packs().borrow_mut();
                    if !roots.contains(&dir) {
                        roots.push(dir);
                    }
                }
                Err(err) => api.notify(format!("pack: {}: {err}", spec.name)),
            }
        }
        Ok(())
    })
}

pub(crate) fn list(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(move |lua, ()| {
        let out = lua.create_table()?;
        for root in api.packs().borrow().iter() {
            out.push(root.display().to_string())?;
        }
        Ok(out)
    })
}

pub(crate) fn update(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(move |_, ()| {
        update_all(&api);
        Ok(())
    })
}

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let pack = lua.create_table()?;
    pack.set("add", add(lua, api)?)?;
    pack.set("list", list(lua, api)?)?;
    pack.set("update", update(lua, api)?)?;
    Ok(pack)
}
