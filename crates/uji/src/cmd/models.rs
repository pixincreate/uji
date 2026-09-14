use std::collections::HashMap;

use uji_core::llm::{self, Provider};

use super::{Action, Args, Context};

#[derive(Default)]
pub struct Models {
    choices: HashMap<String, Choice>,
}

struct Choice {
    provider_id: String,
    model: String,
}

impl Action for Models {
    fn name(&self) -> &'static str {
        "models"
    }

    fn desc(&self) -> &'static str {
        "pick the model"
    }

    fn start<C: Context>(&mut self, ctx: &mut C, _args: &Args) {
        let current = ctx.get_setting("llm.provider").unwrap_or_default();
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
                    format!("{} · {model}", provider.name)
                } else {
                    model.clone()
                };
                self.choices.insert(
                    label.clone(),
                    Choice {
                        provider_id: provider.id.clone(),
                        model: model.clone(),
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

    fn on_select<C: Context>(&mut self, ctx: &mut C, item: String) {
        let Some(choice) = self.choices.get(&item) else {
            ctx.finish();
            return;
        };
        let provider_id = choice.provider_id.clone();
        let model = choice.model.clone();
        if ctx.get_setting("llm.provider").as_deref() != Some(provider_id.as_str()) {
            ctx.set_setting("llm.provider", &provider_id);
            ctx.set_setting("llm.base_url", "");
        }
        super::remember_model(ctx, &provider_id, &model);
        ctx.resolve_llm();
        ctx.finish();
    }

    fn on_cancel<C: Context>(&mut self, ctx: &mut C) {
        ctx.finish();
    }
}
