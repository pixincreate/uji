use mlua::{Function, Table, Value as LuaValue};
use uji_agent::session::model::Message;

use super::LoopData;
use crate::runtime::events;

const PROMPT: &str = "uji.prompt";

impl LoopData {
    pub(super) fn prompt(&self, text: &str, messages: &mut Vec<Message>) -> String {
        let mut system = self.base_prompt().unwrap_or_else(|err| {
            self.inner.report(format!("{PROMPT}: {err}"));
            String::new()
        });
        self.gather_context(&mut system, messages);
        self.before_turn(system, text)
    }

    fn base_prompt(&self) -> mlua::Result<String> {
        let env = self.inner.lua.create_table()?;
        env.set("directory", self.app.session().directory.as_str())?;
        env.set("os", std::env::consts::OS)?;
        self.inner
            .require::<Table>(PROMPT)?
            .get::<Function>("system")?
            .call(env)
    }

    fn gather_context(&self, system: &mut String, messages: &mut Vec<Message>) {
        for (name, call) in self.inner.api.agent_context().borrow().calls() {
            let (text, at_turn) = match call.call::<LuaValue>(()) {
                Ok(LuaValue::String(text)) => (text.to_string_lossy(), false),
                Ok(LuaValue::Table(table)) => {
                    let text = table.get::<Option<String>>("text").unwrap_or_default();
                    let at = table.get::<Option<String>>("at").unwrap_or_default();
                    (text.unwrap_or_default(), at.as_deref() == Some("turn"))
                }
                Ok(_) => continue,
                Err(err) => {
                    self.inner.report(format!("agent context {name}: {err}"));
                    continue;
                }
            };
            if text.trim().is_empty() {
                continue;
            }
            if at_turn {
                messages.push(Message::User { text });
            } else {
                system.push_str("\n\n");
                system.push_str(&text);
            }
        }
    }

    fn before_turn(&self, system: String, text: &str) -> String {
        let turn = match self.inner.lua.create_table() {
            Ok(turn) => turn,
            Err(err) => {
                self.inner.report(format!("before_turn: {err}"));
                return system;
            }
        };
        let _ = turn.set("system", system.as_str());
        let _ = turn.set("text", text);
        self.inner
            .api
            .dispatch(events::Event::BeforeTurn.name(), &turn);
        turn.get::<String>("system").unwrap_or_else(|err| {
            self.inner
                .report(format!("before_turn left no system prompt: {err}"));
            system
        })
    }
}
