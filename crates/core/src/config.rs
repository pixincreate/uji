use std::path::PathBuf;

pub const INIT_FILE: &str = "init.lua";
pub const MODULE_DIR: &str = "lua";
pub const PLUGIN_DIR: &str = "plugin";

pub fn config_dir() -> Option<PathBuf> {
    dir("UJI_CONFIG_DIR", "XDG_CONFIG_HOME", ".config")
}

pub fn init_path(config_dir: &std::path::Path) -> Option<PathBuf> {
    let path = config_dir.join(INIT_FILE);
    path.is_file().then_some(path)
}

pub fn data_dir() -> Option<PathBuf> {
    dir("UJI_DATA_DIR", "XDG_DATA_HOME", ".local/share")
}

pub fn site_dir() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join("site"))
}

pub fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    PathBuf::from(path)
}

fn dir(own: &str, xdg: &str, fallback: &str) -> Option<PathBuf> {
    if let Some(dir) = var(own) {
        return Some(PathBuf::from(dir));
    }
    let base = var(xdg)
        .map(PathBuf::from)
        .filter(|base| base.is_absolute())
        .or_else(|| var("HOME").map(|home| PathBuf::from(home).join(fallback)))?;
    Some(base.join("uji"))
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}
