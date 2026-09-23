use std::collections::HashMap;

use uji_core::llm::{self, Catalog, Provider};

use super::{Action, Args, Context};
use uji_core::session::store::Setting;

#[derive(Default)]
pub struct Models {
    choices: HashMap<String, Choice>,
}

struct Choice {
    provider_id: String,
    model: String,
}

impl Action for Models {
    fn start(&mut self, ctx: &mut dyn Context, _args: &Args) {
        let current = ctx.get_setting(&Setting::Provider).unwrap_or_default();
        let listed = self.list(&ctx.catalog(), &current);
        if let Some((title, items)) = listed {
            ctx.open_select(title, items);
        } else {
            ctx.notify("Please run /login to configure a provider");
            ctx.finish();
        }
    }

    fn on_select(&mut self, ctx: &mut dyn Context, item: String) {
        let Some(choice) = self.choices.get(&item) else {
            ctx.finish();
            return;
        };
        let provider_id = choice.provider_id.clone();
        let model = choice.model.clone();
        if ctx.get_setting(&Setting::Provider).as_deref() != Some(provider_id.as_str()) {
            ctx.set_setting(&Setting::Provider, &provider_id);
            ctx.set_setting(&Setting::BaseUrl, "");
        }
        super::remember_model(ctx, &provider_id, &model);
        ctx.resolve_llm();
        ctx.finish();
    }

    fn on_cancel(&mut self, ctx: &mut dyn Context) {
        ctx.finish();
    }
}

impl Models {
    fn list(&mut self, catalog: &Catalog, current: &str) -> Option<(String, Vec<String>)> {
        let available: Vec<&Provider> = catalog
            .all()
            .iter()
            .filter(|provider| !provider.models.is_empty())
            .filter(|provider| provider.id == current || llm::authenticated(provider))
            .collect();
        let first = available.first()?;
        let qualify = available.len() > 1;
        self.choices.clear();
        let mut items = Vec::new();
        for provider in &available {
            for model in &provider.models {
                let label = if qualify {
                    format!("{} · {}", provider.name, model.id)
                } else {
                    model.id.clone()
                };
                self.choices.insert(
                    label.clone(),
                    Choice {
                        provider_id: provider.id.clone(),
                        model: model.id.clone(),
                    },
                );
                items.push(label);
            }
        }
        let title = if qualify {
            "Models".to_string()
        } else {
            format!("{} models", first.name)
        };
        Some((title, items))
    }
}
