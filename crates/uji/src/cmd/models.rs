use std::collections::HashMap;

use uji_agent::llm::{self, Provider};

use super::{Action, Args, Context};
use uji_agent::session::store::Setting;

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
        let available: Vec<Provider> = ctx
            .providers()
            .into_iter()
            .filter(|provider| !provider.models.is_empty())
            .filter(|provider| provider.id == current || llm::authenticated(provider))
            .collect();

        if available.is_empty() {
            ctx.notify("Please run /login to configure a provider");
            ctx.finish();
            return;
        }

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
            format!("{} models", available[0].name)
        };
        ctx.open_select(title, items);
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
