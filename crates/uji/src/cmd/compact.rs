use super::{Action, Args, Context};

#[derive(Default)]
pub struct Compact;

impl Action for Compact {
    fn start(&mut self, ctx: &mut dyn Context, _args: &Args) {
        if !ctx.compact() {
            ctx.notify("nothing to compact yet");
        }
        ctx.finish();
    }
}
