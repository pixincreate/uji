use super::{Action, Args, Context};
use uji_tui::app::Echo;

const SUBSCRIPTION: &str = "Subscription (sign in with browser)";
const API_KEY: &str = "API key";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Step {
    #[default]
    Provider,
    Method,
    BaseUrl,
    Model,
    ApiKey,
}

#[derive(Default)]
struct Draft {
    provider_id: String,
    is_custom: bool,
    has_oauth: bool,
    auth_env: Vec<String>,
    base_url: String,
    model: String,
}

#[derive(Default)]
pub struct Login {
    step: Step,
    draft: Draft,
}

impl Action for Login {
    fn name(&self) -> &'static str {
        "login"
    }

    fn desc(&self) -> &'static str {
        "configure provider and auth"
    }

    fn start<C: Context>(&mut self, ctx: &mut C, _args: &Args) {
        self.step = Step::Provider;
        self.draft = Draft::default();
        let names = ctx.providers().iter().map(|p| p.name.clone()).collect();
        ctx.open_select("Provider".into(), names);
    }

    fn on_select<C: Context>(&mut self, ctx: &mut C, item: String) {
        match self.step {
            Step::Provider => self.on_provider(ctx, &item),
            Step::Method => self.on_method(ctx, &item),
            _ => {}
        }
    }

    fn on_prompt<C: Context>(&mut self, ctx: &mut C, value: String) {
        match self.step {
            Step::BaseUrl => {
                self.draft.base_url = value;
                self.step = Step::Model;
                ctx.open_prompt("model".into(), String::new(), Echo::Plain);
            }
            Step::Model => {
                self.draft.model = value;
                self.ask_api_key(ctx);
            }
            Step::ApiKey => {
                if !value.is_empty() {
                    ctx.save_credential(&self.draft.provider_id, &value);
                }
                self.finish_configure(ctx);
            }
            Step::Provider | Step::Method => {}
        }
    }

    fn on_cancel<C: Context>(&mut self, ctx: &mut C) {
        ctx.finish();
    }
}

impl Login {
    fn on_provider<C: Context>(&mut self, ctx: &mut C, item: &str) {
        let Some(provider) = ctx.provider_by_name(item) else {
            ctx.finish();
            return;
        };
        provider.id.clone_into(&mut self.draft.provider_id);
        self.draft.is_custom = provider.id == "custom";
        self.draft.has_oauth = provider.oauth.is_some();
        self.draft.auth_env.clone_from(&provider.auth_env);

        if self.draft.has_oauth {
            self.step = Step::Method;
            ctx.open_select(
                format!("{} sign-in", provider.name),
                vec![SUBSCRIPTION.to_string(), API_KEY.to_string()],
            );
        } else if self.draft.is_custom {
            self.ask_base_url(ctx);
        } else if self.draft.auth_env.is_empty() {
            self.finish_configure(ctx);
        } else {
            self.ask_api_key(ctx);
        }
    }

    fn on_method<C: Context>(&mut self, ctx: &mut C, item: &str) {
        if item == SUBSCRIPTION {
            self.finish_configure(ctx);
            ctx.start_oauth(&self.draft.provider_id.clone());
        } else {
            self.ask_api_key(ctx);
        }
    }

    fn ask_base_url<C: Context>(&mut self, ctx: &mut C) {
        self.step = Step::BaseUrl;
        ctx.open_prompt("base_url".into(), String::new(), Echo::Plain);
    }

    fn ask_api_key<C: Context>(&mut self, ctx: &mut C) {
        self.step = Step::ApiKey;
        let title = if self.draft.auth_env.is_empty() {
            "api_key (enter to skip)".to_string()
        } else {
            format!("{} (enter to skip)", self.draft.auth_env.join(" or "))
        };
        ctx.open_prompt(title, String::new(), Echo::Hidden);
    }

    fn finish_configure<C: Context>(&self, ctx: &mut C) {
        ctx.set_setting("llm.provider", &self.draft.provider_id);
        if self.draft.is_custom {
            ctx.set_setting("llm.base_url", &self.draft.base_url);
            let model = self.draft.model.clone();
            super::remember_model(ctx, &self.draft.provider_id, &model);
        } else {
            ctx.set_setting("llm.base_url", "");
            if let Some(provider) = ctx.provider(&self.draft.provider_id) {
                let model = super::model_for(ctx, &provider);
                super::remember_model(ctx, &provider.id, &model);
            }
        }
        ctx.resolve_llm();
        ctx.notify(&format!("logged in to {}", self.draft.provider_id));
        ctx.finish();
    }
}
