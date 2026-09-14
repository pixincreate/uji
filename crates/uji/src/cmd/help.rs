use super::{Action, Args, Context};

pub struct Help;

impl Action for Help {
    fn start(&mut self, ctx: &mut dyn Context, _args: &Args) {
        ctx.open_select("Commands".into(), ctx.command_names());
    }

    fn on_select(&mut self, ctx: &mut dyn Context, _item: String) {
        ctx.finish();
    }

    fn on_cancel(&mut self, ctx: &mut dyn Context) {
        ctx.finish();
    }
}
