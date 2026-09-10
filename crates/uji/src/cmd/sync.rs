use super::{Action, Args, Context};

#[derive(Default)]
pub struct Sync;

impl Action for Sync {
    fn name(&self) -> &'static str {
        "sync"
    }

    fn desc(&self) -> &'static str {
        "update installed packs"
    }

    fn start<C: Context>(&mut self, ctx: &mut C, _args: &Args) {
        ctx.sync_packs();
        ctx.finish();
    }
}
