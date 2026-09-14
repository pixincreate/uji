pub mod help;
pub mod login;
pub mod models;
pub mod reload;
pub mod sync;

pub use help::Help;
pub use login::Login;
pub use models::Models;
pub use reload::Reload;
pub use sync::Sync;

pub struct Args {
    pub raw: String,
    pub tokens: Vec<String>,
}

impl Args {
    pub fn parse(raw: &str) -> Self {
        let raw = raw.trim().to_string();
        let tokens = raw.split_whitespace().map(str::to_string).collect();
        Self { raw, tokens }
    }
}
use uji_core::llm::Provider;
use uji_tui::app::Echo;

pub trait Context {
    fn open_select(&mut self, title: String, items: Vec<String>);
    fn open_prompt(&mut self, title: String, value: String, echo: Echo);
    fn set_setting(&mut self, key: &str, value: &str);
    fn get_setting(&mut self, key: &str) -> Option<String>;
    fn save_credential(&mut self, provider: &str, key: &str);
    fn resolve_llm(&mut self);
    fn reload(&mut self);
    fn sync_packs(&mut self);
    fn start_oauth(&mut self, provider_id: &str);
    fn notify(&mut self, message: &str);
    fn finish(&mut self);
    fn providers(&self) -> Vec<Provider>;
    fn provider(&self, id: &str) -> Option<Provider>;
    fn provider_by_name(&self, name: &str) -> Option<Provider>;
    fn command_names(&self) -> Vec<String>;
}

pub(crate) fn remember_model<C: Context>(ctx: &mut C, provider_id: &str, model: &str) {
    ctx.set_setting("llm.model", model);
    ctx.set_setting(&format!("llm.model.{provider_id}"), model);
}

pub(crate) fn model_for<C: Context>(ctx: &mut C, provider: &Provider) -> String {
    let stored = ctx
        .get_setting(&format!("llm.model.{}", provider.id))
        .or_else(|| ctx.get_setting("llm.model"))
        .filter(|model| !model.is_empty());
    if provider.models.is_empty() {
        return stored.unwrap_or_default();
    }
    stored
        .filter(|model| provider.models.iter().any(|known| known == model))
        .unwrap_or_else(|| provider.default_model().to_string())
}

pub trait Action {
    fn name(&self) -> &'static str;
    fn desc(&self) -> &'static str {
        ""
    }
    fn start<C: Context>(&mut self, ctx: &mut C, args: &Args);
    fn on_select<C: Context>(&mut self, _ctx: &mut C, _item: String) {}
    fn on_prompt<C: Context>(&mut self, _ctx: &mut C, _value: String) {}
    fn on_cancel<C: Context>(&mut self, _ctx: &mut C) {}
}
