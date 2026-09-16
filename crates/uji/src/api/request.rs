use crate::api::modal;

/// Something a plugin asked the runtime to do, to be carried out on the next
/// loop tick.
///
/// One ordered queue rather than a slot per kind, so requests run in the order
/// the plugin made them.
pub enum Request {
    Submit(String),
    SetTitle(String),
    Exec(Vec<String>),
    Interrupt,
    Modal(Box<modal::ModalRequest>),
    Answer(modal::Answer),
}
