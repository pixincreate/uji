use mlua::Function;

use super::{Action, Args, Context};

pub(crate) struct LuaAction {
    on_done: Function,
}

impl LuaAction {
    pub(crate) fn new(on_done: Function) -> Self {
        Self { on_done }
    }

    fn done(&self, ctx: &mut dyn Context, answer: Option<String>) {
        if let Err(err) = self.on_done.call::<()>(answer) {
            ctx.notify(&format!("modal callback error: {err}"));
        }
        ctx.finish();
    }
}

impl Action for LuaAction {
    fn start(&mut self, _ctx: &mut dyn Context, _args: &Args) {}

    fn on_select(&mut self, ctx: &mut dyn Context, item: String) {
        self.done(ctx, Some(item));
    }

    fn on_prompt(&mut self, ctx: &mut dyn Context, value: String) {
        self.done(ctx, Some(value));
    }

    fn on_cancel(&mut self, ctx: &mut dyn Context) {
        self.done(ctx, None);
    }
}
