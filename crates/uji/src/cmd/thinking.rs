use super::{Action, Args, Context};

#[derive(Default)]
pub struct Thinking;

impl Action for Thinking {
    fn start(&mut self, ctx: &mut dyn Context, _args: &Args) {
        ctx.toggle_thinking();
        ctx.finish();
    }
}
