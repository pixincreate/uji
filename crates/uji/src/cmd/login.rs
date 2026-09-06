use super::{Action, Args, Context};
use crate::llm;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Step {
    #[default]
    Provider,
    BaseUrl,
    Model,
    ApiKey,
}

#[derive(Default)]
struct Draft {
    provider_id: String,
    is_custom: bool,
    auth_env: Option<String>,
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
        let names = llm::providers().iter().map(|p| p.name.clone()).collect();
        ctx.open_select("Provider".into(), names);
    }

    fn on_select<C: Context>(&mut self, ctx: &mut C, item: String) {
        if self.step != Step::Provider {
            return;
        }
        let Some(provider) = llm::provider_by_name(&item) else {
            ctx.finish();
            return;
        };
        provider.id.clone_into(&mut self.draft.provider_id);
        self.draft.is_custom = provider.id == "custom";
        provider.auth_env.clone_into(&mut self.draft.auth_env);
        if self.draft.is_custom {
            self.step = Step::BaseUrl;
            ctx.open_prompt("base_url".into(), String::new(), false);
        } else if self.draft.auth_env.is_some() {
            self.step = Step::ApiKey;
            let title = format!(
                "{} (enter to skip)",
                self.draft.auth_env.as_deref().unwrap_or("")
            );
            ctx.open_prompt(title, String::new(), true);
        } else {
            self.finish_configure(ctx);
        }
    }

    fn on_prompt<C: Context>(&mut self, ctx: &mut C, value: String) {
        match self.step {
            Step::BaseUrl => {
                self.draft.base_url = value;
                self.step = Step::Model;
                ctx.open_prompt("model".into(), String::new(), false);
            }
            Step::Model => {
                self.draft.model = value;
                self.step = Step::ApiKey;
                ctx.open_prompt("api_key (enter to skip)".into(), String::new(), true);
            }
            Step::ApiKey => {
                if !value.is_empty() {
                    ctx.save_credential(&self.draft.provider_id, &value);
                }
                self.finish_configure(ctx);
            }
            Step::Provider => {}
        }
    }

    fn on_cancel<C: Context>(&mut self, ctx: &mut C) {
        ctx.finish();
    }
}

impl Login {
    fn finish_configure<C: Context>(&self, ctx: &mut C) {
        ctx.set_setting("llm.provider", &self.draft.provider_id);
        if self.draft.is_custom {
            ctx.set_setting("llm.base_url", &self.draft.base_url);
            ctx.set_setting("llm.model", &self.draft.model);
        } else {
            ctx.set_setting("llm.base_url", "");
            if ctx.get_setting("llm.model").is_none()
                && let Some(provider) = llm::provider(&self.draft.provider_id)
            {
                ctx.set_setting("llm.model", provider.default_model());
            }
        }
        ctx.resolve_llm();
        ctx.notify(&format!("logged in to {}", self.draft.provider_id));
        ctx.finish();
    }
}
