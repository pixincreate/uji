use strum::VariantArray;
use uji_core::llm::Effort;

use super::{Action, Args, Context};
use uji_core::session::store::Setting;

#[derive(Default)]
pub struct EffortPick;

impl Action for EffortPick {
    fn start(&mut self, ctx: &mut dyn Context, _args: &Args) {
        let current = ctx.get_setting(&Setting::Effort).unwrap_or_default();
        let items = Effort::VARIANTS
            .iter()
            .copied()
            .map(|effort| {
                if effort.name() == current {
                    format!("{effort} (current)")
                } else {
                    effort.to_string()
                }
            })
            .collect();
        ctx.open_select("Reasoning effort".into(), items);
    }

    fn on_select(&mut self, ctx: &mut dyn Context, item: String) {
        let name = item.split_whitespace().next().unwrap_or_default();
        match Effort::parse(name) {
            Some(effort) => {
                ctx.set_setting(&Setting::Effort, effort.name());
                ctx.resolve_llm();
                ctx.notify(&format!("reasoning effort: {effort}"));
            }
            None => ctx.notify("unknown effort"),
        }
        ctx.finish();
    }

    fn on_cancel(&mut self, ctx: &mut dyn Context) {
        ctx.finish();
    }
}
