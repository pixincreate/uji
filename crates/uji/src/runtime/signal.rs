use uji_core::llm::Call;

use super::auth::AuthEvent;
use super::background::{CompactEvent, ModelsEvent, TitleEvent};
use super::job::JobEvent;
use super::loop_data::shell::ShellEvent;
use super::reply::Reply;

pub(crate) enum Signal {
    Job(JobEvent),
    Reply(Reply),
    Line { id: u64, line: String },
    Wire(Call),
    Shell(ShellEvent),
    Auth(AuthEvent),
    Title(TitleEvent),
    Compacted(CompactEvent),
    Models(ModelsEvent),
}
