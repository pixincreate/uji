use std::sync::Arc;

use super::LoopData;
use crate::runtime::auth::AuthEvent;
use uji_core::llm::Origin;

use crate::runtime::background::{self, ModelsEvent, TitleEvent};
use crate::runtime::events;
use crate::runtime::signal::Signal;

impl LoopData {
    pub(crate) fn on_signal(&mut self, signal: Signal) {
        match signal {
            Signal::Job(event) => self.on_job_event(&event),
            Signal::Reply(reply) => self.on_reply(reply),
            Signal::Line { id, line } => self.on_line(id, &line),
            Signal::Wire(call) => self.on_wire_call(call),
            Signal::Shell(event) => self.on_shell_event(event),
            Signal::Auth(event) => self.on_auth_event(event),
            Signal::Title(event) => self.on_title_event(event),
            Signal::Compacted(event) => self.on_compacted(event),
            Signal::Models(event) => self.on_models(&event),
        }
    }

    pub(super) fn set_title(&mut self, title: &str) {
        let id = self.app.messages().info().id;
        if let Err(err) = self.storage.rename_session(&id, title) {
            self.inner
                .notify(format!("could not save the session title: {err}"));
            return;
        }
        self.app
            .conversation()
            .borrow_mut()
            .set_title(title.to_string());
        self.inner.emit(&events::SessionTitled { title });
        self.dirty = true;
    }

    pub(super) fn maybe_title(&mut self, first_message: &str) {
        if !self.app.messages().info().is_untitled() {
            return;
        }
        if self.app.conversation().borrow().messages().len() != 1 {
            return;
        }
        background::title(
            &self.work,
            Arc::clone(&self.inner.client),
            Arc::clone(&self.inner.llm),
            self.inner.llm_model.clone(),
            first_message.to_string(),
        );
    }

    fn on_title_event(&mut self, event: TitleEvent) {
        let TitleEvent::Ready { title, usage } = event else {
            return;
        };
        if let Some(usage) = usage {
            self.app.conversation().borrow_mut().add_cost(usage);
        }
        self.set_title(&title);
    }

    fn on_auth_event(&mut self, event: AuthEvent) {
        match event {
            AuthEvent::Opened { url } => {
                self.inner
                    .notify(format!("opened your browser to sign in - {url}"));
            }
            AuthEvent::Done { provider_id } => {
                self.inner.notify(format!("signed in to {provider_id}"));
                self.inner.resolve_llm(&mut *self.storage);
                self.inner.emit(&events::StatusChanged);
            }
            AuthEvent::Failed { message } => {
                self.inner.notify(format!("sign-in failed: {message}"));
            }
        }
        self.drain_notices();
        self.dirty = true;
    }
}

impl LoopData {
    pub(super) fn discover_model_windows(&mut self) {
        let registered: Vec<_> = self
            .inner
            .api
            .providers()
            .borrow()
            .all()
            .iter()
            .filter(|provider| {
                provider.origin == Origin::Registered && !provider.base_url.is_empty()
            })
            .cloned()
            .collect();
        if registered.is_empty() {
            return;
        }
        background::model_windows(&self.work, &self.inner.client, registered);
    }

    fn on_models(&mut self, event: &ModelsEvent) {
        let changed = self
            .inner
            .api
            .providers()
            .borrow_mut()
            .set_windows(&event.provider, &event.windows);
        if changed == 0 {
            return;
        }
        self.inner.notify(format!(
            "{}: corrected {changed} model window{} from the endpoint",
            event.provider,
            if changed == 1 { "" } else { "s" }
        ));
        self.drain_notices();
        self.dirty = true;
    }
}
