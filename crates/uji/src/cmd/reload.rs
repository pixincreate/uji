use super::{Action, Args, Context};

#[derive(Default)]
pub struct Reload;

impl Action for Reload {
    fn name(&self) -> &'static str {
        "reload"
    }

    fn desc(&self) -> &'static str {
        "redraw the UI from config"
    }

    fn start<C: Context>(&mut self, ctx: &mut C, _args: &Args) {
        ctx.reload();
        ctx.finish();
    }
}
