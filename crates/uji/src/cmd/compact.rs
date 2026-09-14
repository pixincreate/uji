use super::{Action, Args, Context};

#[derive(Default)]
pub struct Compact;

impl Action for Compact {
    fn name(&self) -> &'static str {
        "compact"
    }

    fn desc(&self) -> &'static str {
        "summarise earlier messages to free context"
    }

    fn start<C: Context>(&mut self, ctx: &mut C, _args: &Args) {
        if !ctx.compact() {
            ctx.notify("nothing to compact yet");
        }
        ctx.finish();
    }
}
