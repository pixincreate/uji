use std::path::PathBuf;

pub const INIT_FILE: &str = "init.lua";
pub const MODULE_DIR: &str = "lua";
pub const PLUGIN_DIR: &str = "plugin";

pub fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var("UJI_CONFIG_DIR")
        .ok()
        .filter(|dir| !dir.is_empty())
    {
        return Some(PathBuf::from(dir));
    }
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".config/uji"))
}

pub fn init_path(config_dir: &std::path::Path) -> Option<PathBuf> {
    let path = config_dir.join(INIT_FILE);
    path.is_file().then_some(path)
}

pub fn data_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var("UJI_DATA_DIR")
        .ok()
        .filter(|dir| !dir.is_empty())
    {
        return Some(PathBuf::from(dir));
    }
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".local/share/uji"))
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
