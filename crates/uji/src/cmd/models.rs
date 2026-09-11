use super::{Action, Args, Context};
use uji_core::llm;

#[derive(Default)]
pub struct Models;

impl Action for Models {
    fn name(&self) -> &'static str {
        "models"
    }

    fn desc(&self) -> &'static str {
        "pick the default model"
    }

    fn start<C: Context>(&mut self, ctx: &mut C, _args: &Args) {
        let provider_id = ctx.get_setting("llm.provider").unwrap_or_default();
        let Some(provider) = llm::provider(&provider_id) else {
            ctx.notify("Please run /login to configure a provider");
            ctx.finish();
            return;
        };
        let model_ids = llm::models(&provider_id);
        if model_ids.is_empty() {
            ctx.notify(&format!("no models listed for {}", provider.name));
            ctx.finish();
            return;
        }
        let items = model_ids.iter().map(|model| (*model).to_string()).collect();
        ctx.open_select(format!("{} models", provider.name), items);
    }

    fn on_select<C: Context>(&mut self, ctx: &mut C, item: String) {
        ctx.set_setting("llm.model", &item);
        ctx.resolve_llm();
        ctx.finish();
    }

    fn on_cancel<C: Context>(&mut self, ctx: &mut C) {
        ctx.finish();
    }
}
