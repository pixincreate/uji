use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use uji_engine::config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub(crate) url: String,
    pub(crate) rev: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) reference: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Lock {
    #[serde(flatten)]
    packs: BTreeMap<String, Entry>,
}

impl Lock {
    pub(crate) fn path() -> Option<PathBuf> {
        config::data_dir().map(|dir| dir.join("uji-lock.json"))
    }

    pub(crate) fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub(crate) fn get(&self, name: &str) -> Option<&Entry> {
        self.packs.get(name)
    }

    pub(crate) fn set(&mut self, name: String, entry: Entry) {
        self.packs.insert(name, entry);
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = (&String, &Entry)> {
        self.packs.iter()
    }

    pub(crate) fn save(&self) -> Result<(), String> {
        let Some(path) = Self::path() else {
            return Err(String::from("$HOME is not set"));
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("mkdir {}: {err}", parent.display()))?;
        }
        let text =
            serde_json::to_string_pretty(self).map_err(|err| format!("encode lockfile: {err}"))?;
        std::fs::write(&path, text).map_err(|err| format!("write {}: {err}", path.display()))
    }
}
