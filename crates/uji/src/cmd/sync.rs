use super::{Action, Args, Context};

#[derive(Default)]
pub struct Sync;

impl Action for Sync {
    fn start(&mut self, ctx: &mut dyn Context, _args: &Args) {
        ctx.sync_packs();
        ctx.finish();
    }
}
