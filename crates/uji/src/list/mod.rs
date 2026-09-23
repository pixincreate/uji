use std::error::Error;

use uji_ui::render::style::Palette;

use crate::runtime::{Runtime, events};
use uji_core::session::model::Session;
use uji_core::session::store::SessionStorage;

pub fn run(mut storage: Box<dyn SessionStorage>) -> Result<(), Box<dyn Error>> {
    let current_dir = std::env::current_dir().map_or_else(
        |_| String::new(),
        |path| path.to_string_lossy().into_owned(),
    );

    let sessions: Vec<Session> = storage
        .list_sessions()?
        .into_iter()
        .filter(|session| session.directory == current_dir)
        .collect();

    // Boot before picking so the picker is drawn with the user's theme.
    let runtime = Runtime::boot()?;
    let palette = Palette::of(&runtime.state().borrow().opts().theme);

    let Some(index) = uji_ui::sessions::pick(&sessions, &current_dir, palette)? else {
        return Ok(());
    };

    let session = &sessions[index];
    runtime.emit(&events::SessionResumed {
        session_id: session.id,
    });
    runtime.run(session, storage)?;
    Ok(())
}
