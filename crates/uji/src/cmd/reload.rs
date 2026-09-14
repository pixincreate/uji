use super::{Action, Args, Context};

#[derive(Default)]
pub struct Reload;

impl Action for Reload {
    fn start(&mut self, ctx: &mut dyn Context, _args: &Args) {
        ctx.reload();
        ctx.finish();
    }
}
