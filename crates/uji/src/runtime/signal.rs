use uji_core::llm::StreamEvent;

use super::auth::AuthEvent;
use super::background::{CompactEvent, TitleEvent};
use super::job::JobEvent;

pub(crate) enum Signal {
    Llm(StreamEvent),
    Job(JobEvent),
    Auth(AuthEvent),
    Title(TitleEvent),
    Compacted(CompactEvent),
}
