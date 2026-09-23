use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use crate::credential;
use crate::llm::discover;
use crate::session::store::{SessionStorage, Setting};

use super::bridge::{Bridge, Dispatch};
use super::tuning::{Effort, Retention};
use super::{Llm, NotConfigured};

const DEFAULT_WIRE: &str = "openai-chat";

#[derive(Default)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    pub compat: Value,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub auth_env: Vec<String>,
    pub oauth: Option<OAuthSession>,
}

#[derive(Clone)]
pub struct OAuthSession {
    pub provider_id: String,
    pub config: crate::auth::OAuthConfig,
    pub tokens: crate::auth::Tokens,
}

impl LlmConfig {
    pub fn for_provider(
        provider_id: String,
        known: Option<&Provider>,
        model: String,
        base_url: Option<String>,
    ) -> Self {
        let catalog_url = known
            .map(|entry| entry.base_url.clone())
            .filter(|url| !url.is_empty());
        Self {
            compat: known.map(|entry| entry.compat.clone()).unwrap_or_default(),
            base_url: base_url.filter(|url| !url.is_empty()).or(catalog_url),
            auth_env: known
                .map(|entry| entry.auth_env.clone())
                .unwrap_or_default(),
            oauth: oauth_session(&provider_id, known),
            api_key: credential::get(&provider_id),
            provider: provider_id,
            model,
        }
    }

    pub fn resolve_key(&self) -> Option<String> {
        self.api_key.clone().or_else(|| env_key(&self.auth_env))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub wire: String,
    pub base_url: String,
    #[serde(default)]
    pub compat: Value,
    #[serde(default)]
    pub auth_env: Vec<String>,
    #[serde(default)]
    pub oauth: Option<crate::auth::OAuthConfig>,
    #[serde(default)]
    pub context_window: Option<u64>,
    #[serde(default)]
    pub models: Vec<Model>,
    #[serde(skip)]
    pub origin: Origin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Origin {
    #[default]
    Builtin,
    Registered,
}

pub const MAX_RESERVE: u64 = 20_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(from = "ModelSpec")]
pub struct Model {
    pub id: String,
    pub context: Option<u64>,
    pub output: Option<u64>,
    pub reasoning: bool,
    pub cache: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ModelSpec {
    Id(String),
    Full {
        id: String,
        #[serde(default)]
        context: Option<u64>,
        #[serde(default)]
        output: Option<u64>,
        #[serde(default)]
        reasoning: bool,
        #[serde(default)]
        cache: bool,
    },
}

impl From<ModelSpec> for Model {
    fn from(spec: ModelSpec) -> Self {
        match spec {
            ModelSpec::Id(id) => Self {
                id,
                context: None,
                output: None,
                reasoning: false,
                cache: false,
            },
            ModelSpec::Full {
                id,
                context,
                output,
                reasoning,
                cache,
            } => Self {
                id,
                context,
                output,
                reasoning,
                cache,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub window: u64,
    pub reserve: u64,
}

impl Budget {
    pub fn usable(self) -> u64 {
        self.window.saturating_sub(self.reserve)
    }

    pub fn overflows(self, used: u64) -> bool {
        used >= self.usable()
    }
}

impl Provider {
    pub fn default_model(&self) -> &str {
        self.models.first().map_or("", |model| model.id.as_str())
    }

    pub fn model(&self, id: &str) -> Option<&Model> {
        self.models.iter().find(|model| model.id == id)
    }

    pub fn usable_model(&self, stored: Option<String>) -> String {
        let stored = stored.filter(|model| !model.is_empty());
        if self.models.is_empty() {
            return stored.unwrap_or_default();
        }
        stored
            .filter(|model| self.model(model).is_some())
            .unwrap_or_else(|| self.default_model().to_string())
    }

    pub fn reasons(&self, model_id: &str) -> bool {
        self.model(model_id).is_some_and(|model| model.reasoning)
    }

    pub fn caches(&self, model_id: &str) -> bool {
        self.model(model_id).is_some_and(|model| model.cache)
    }

    pub fn budget(&self, model_id: &str) -> Option<Budget> {
        let known = self.model(model_id);
        let window = known
            .and_then(|model| model.context)
            .or(self.context_window)?;
        let reserve = known
            .and_then(|model| model.output)
            .unwrap_or(MAX_RESERVE)
            .min(window / 4);
        Some(Budget { window, reserve })
    }
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    providers: Vec<Provider>,
}

impl Catalog {
    pub fn new(providers: Vec<Provider>) -> Self {
        Self { providers }
    }

    pub fn add(&mut self, mut provider: Provider) {
        provider.origin = Origin::Registered;
        match self
            .providers
            .iter_mut()
            .find(|entry| entry.id == provider.id)
        {
            Some(entry) => *entry = provider,
            None => self.providers.push(provider),
        }
    }

    pub fn remove(&mut self, id: &str) {
        self.providers.retain(|entry| entry.id != id);
    }

    pub fn get(&self, id: &str) -> Option<&Provider> {
        self.providers.iter().find(|entry| entry.id == id)
    }

    pub fn by_name(&self, name: &str) -> Option<&Provider> {
        self.providers.iter().find(|entry| entry.name == name)
    }

    pub fn all(&self) -> &[Provider] {
        &self.providers
    }

    pub fn set_windows(&mut self, id: &str, windows: &[discover::Windows]) -> usize {
        let Some(provider) = self.providers.iter_mut().find(|entry| entry.id == id) else {
            return 0;
        };
        let mut changed = 0;
        for found in windows {
            let Some(model) = provider
                .models
                .iter_mut()
                .find(|model| model.id == found.model)
            else {
                continue;
            };
            if model.context == found.context && model.output == found.output {
                continue;
            }
            model.context = found.context.or(model.context);
            model.output = found.output.or(model.output);
            changed += 1;
        }
        changed
    }
}

fn env_key(names: &[String]) -> Option<String> {
    names
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|key| !key.is_empty())
}

pub fn authenticated(provider: &Provider) -> bool {
    credential::load(&provider.id).is_some() || env_key(&provider.auth_env).is_some()
}

fn oauth_session(provider_id: &str, known: Option<&Provider>) -> Option<OAuthSession> {
    let config = known?.oauth.clone()?;
    let credential::Credential::OAuth {
        access,
        refresh,
        expires_at,
    } = credential::load(provider_id)?
    else {
        return None;
    };
    Some(OAuthSession {
        provider_id: provider_id.to_string(),
        config,
        tokens: crate::auth::Tokens {
            access,
            refresh,
            expires_at,
        },
    })
}

pub fn resolve(wire: Option<&str>, config: &LlmConfig, dispatch: &Dispatch) -> Arc<Llm> {
    if config.provider.is_empty() {
        return Arc::new(Llm::NotConfigured(NotConfigured));
    }
    let wire = wire.unwrap_or(DEFAULT_WIRE).to_string();
    Arc::new(Llm::Wired(Box::new(Bridge::new(
        wire,
        config,
        Arc::clone(dispatch),
    ))))
}

pub struct Selection {
    pub llm: Arc<Llm>,
    pub id: String,
    pub name: String,
    pub model: String,
    pub effort: Effort,
    pub cache: Retention,
}

fn setting(storage: &mut dyn SessionStorage, key: &Setting) -> Option<String> {
    storage.get_setting(key).ok().flatten()
}

pub fn resolve_from_storage(
    storage: &mut dyn SessionStorage,
    catalog: &Catalog,
    dispatch: &Dispatch,
) -> Selection {
    let provider_id = setting(storage, &Setting::Provider).unwrap_or_default();
    let known = catalog.get(&provider_id);
    let stored = setting(storage, &Setting::Model);
    let model = known.map_or_else(
        || stored.clone().unwrap_or_default(),
        |entry| entry.usable_model(stored.clone()),
    );
    let config = LlmConfig::for_provider(
        provider_id.clone(),
        known,
        model.clone(),
        setting(storage, &Setting::BaseUrl),
    );
    let effort = setting(storage, &Setting::Effort)
        .and_then(|name| Effort::parse(&name))
        .filter(|_| known.is_some_and(|entry| entry.reasons(&model)))
        .unwrap_or_default();
    let cache = setting(storage, &Setting::Cache)
        .and_then(|name| Retention::parse(&name))
        .unwrap_or_default();
    let cache = if known.is_some_and(|entry| entry.caches(&model)) {
        cache
    } else {
        Retention::Off
    };
    Selection {
        llm: resolve(known.map(|entry| entry.wire.as_str()), &config, dispatch),
        name: known.map_or_else(|| provider_id.clone(), |entry| entry.name.clone()),
        id: provider_id,
        model,
        effort,
        cache,
    }
}
