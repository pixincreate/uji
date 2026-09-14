use std::sync::Arc;

use uji_core::credential;
use uji_core::llm::Provider;
use uji_tui::app::{Echo, SuggestItem};

use super::{Control, LoopData, ModalInput};
use crate::cmd::{Args, Context};
use crate::runtime::builtin::Builtin;
use crate::runtime::events;

impl LoopData {
    pub(crate) fn refresh_suggestions(&mut self) {
        let pool = suggest_pool(self);
        self.app.set_suggestions(pool);
    }

    pub(super) fn on_command(&mut self, command: &str) {
        let trimmed = command.trim();
        let (name, rest) = match trimmed.find(char::is_whitespace) {
            Some(i) => (&trimmed[..i], trimmed[i..].trim_start()),
            None => (trimmed, ""),
        };
        let args = Args::parse(rest);
        let lua_command = self.inner.api.commands().borrow().get(name).cloned();
        if let Some(mut action) = Builtin::from_name(name) {
            action.start(self, &args);
            self.active = Some(action);
        } else if let Some(handler) = lua_command {
            if let Err(err) = handler.call::<()>((args.raw.clone(),)) {
                self.inner.report(format!("{name}: {err}"));
            }
        } else {
            self.inner.report(format!("unknown command: {name}"));
        }
    }

    pub(super) fn on_modal(&mut self, input: ModalInput) {
        let mut active = self.active.take();
        if let Some(action) = active.as_mut() {
            self.action_done = false;
            match input {
                ModalInput::Select(item) => action.on_select(self, item),
                ModalInput::Prompt(value) => action.on_prompt(self, value),
                ModalInput::Cancel => action.on_cancel(self),
            }
        }
        if let Some(action) = active
            && !self.action_done
        {
            self.active = Some(action);
        }
    }
}

impl Context for LoopData {
    fn open_select(&mut self, title: String, items: Vec<String>) {
        self.app.open_select(title, items);
    }

    fn open_prompt(&mut self, title: String, value: String, echo: Echo) {
        self.app.open_prompt(title, value, echo);
    }

    fn set_setting(&mut self, key: &str, value: &str) {
        let _ = self.storage.set_setting(key, value);
    }

    fn get_setting(&mut self, key: &str) -> Option<String> {
        self.storage.get_setting(key).ok().flatten()
    }

    fn save_credential(&mut self, provider: &str, key: &str) {
        if let Err(err) = credential::set(provider, key) {
            self.inner
                .report(format!("failed to save credential: {err}"));
        }
    }

    fn resolve_llm(&mut self) {
        self.inner.resolve_llm(&mut *self.storage);
        self.inner.emit(events::STATUS_CHANGED, &[]);
        self.dirty = true;
    }

    fn reload(&mut self) {
        self.control = Control::Reload;
    }

    fn compact(&mut self) -> bool {
        let keep = self.keep_recent();
        self.run_compaction(keep)
    }

    fn start_oauth(&mut self, provider_id: &str) {
        let Some(provider) = self
            .inner
            .api
            .providers()
            .borrow()
            .get(provider_id)
            .cloned()
        else {
            self.inner
                .report(format!("unknown provider: {provider_id}"));
            return;
        };
        crate::runtime::auth::start(
            &self.runtime,
            Arc::clone(&self.inner.client),
            &provider,
            self.signals.clone(),
        );
    }

    fn sync_packs(&mut self) {
        crate::pack::update_all(&self.inner.api);
        self.control = Control::Reload;
    }

    fn notify(&mut self, message: &str) {
        self.app.push_notices(vec![message.to_string()]);
        self.dirty = true;
    }

    fn finish(&mut self) {
        self.action_done = true;
    }

    fn providers(&self) -> Vec<Provider> {
        self.inner.api.providers().borrow().all().to_vec()
    }

    fn provider(&self, id: &str) -> Option<Provider> {
        self.inner.api.providers().borrow().get(id).cloned()
    }

    fn provider_by_name(&self, name: &str) -> Option<Provider> {
        self.inner.api.providers().borrow().by_name(name).cloned()
    }

    fn command_names(&self) -> Vec<String> {
        suggest_pool(self)
            .into_iter()
            .map(|item| item.name)
            .collect()
    }
}

fn suggest_pool(data: &LoopData) -> Vec<SuggestItem> {
    let mut items: Vec<SuggestItem> = Builtin::ALL
        .iter()
        .map(|(name, desc)| SuggestItem {
            name: (*name).to_string(),
            desc: (*desc).to_string(),
        })
        .collect();
    let mut lua: Vec<SuggestItem> = data
        .inner
        .api
        .commands()
        .borrow()
        .keys()
        .map(|name| SuggestItem {
            name: name.clone(),
            desc: "lua command".into(),
        })
        .collect();
    items.append(&mut lua);
    items
}
