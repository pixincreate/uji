use std::cell::RefCell;
use std::rc::Rc;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use uji_core::session::conversation::Conversation;
use uji_core::session::id::{MessageId, now_millis};
use uji_core::session::model::{Message, StoredMessage};
use uji_ui::app::{App, Echo};
use uji_ui::keymap::{Chord, Key};
use uji_ui::model::{Builtin, Size, Split, WinOpts};
use uji_ui::render;
use uji_ui::state::UiState;

const CURSOR: char = '\u{2588}';
const MASK: char = '\u{2022}';

fn app() -> App {
    App::new(
        Conversation::shared(),
        Rc::new(RefCell::new(UiState::new())),
    )
}

fn windowed() -> App {
    let app = app();
    {
        let mut state = app.state().borrow_mut();
        state.open_window(
            Some(Builtin::Messages),
            WinOpts {
                size: Size::Fill,
                ..WinOpts::default()
            },
        );
        state.open_window(
            Some(Builtin::Input),
            WinOpts {
                split: Split::Bottom,
                size: Size::Auto,
                ..WinOpts::default()
            },
        );
    }
    app
}

fn screen(app: &App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render::render(frame, app))
        .unwrap()
        .buffer
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

fn typing(app: &mut App, text: &str) {
    for c in text.chars() {
        app.handle_key(Chord::plain(Key::Char(c)));
    }
}

#[test]
fn the_draft_is_drawn_with_the_cursor_in_it() {
    let mut app = windowed();
    typing(&mut app, "hello");
    app.handle_key(Chord::plain(Key::Left));
    app.handle_key(Chord::plain(Key::Left));
    let text = screen(&app, 40, 8);
    assert!(
        text.contains(&format!("hel{CURSOR}lo")),
        "the draft and its cursor should be on screen"
    );
}

#[test]
fn a_hidden_prompt_shows_no_characters() {
    let mut app = windowed();
    app.open_prompt(String::from("api key"), String::new(), Echo::Hidden);
    typing(&mut app, "hunter2");
    app.handle_key(Chord::plain(Key::Left));
    let text = screen(&app, 40, 10);
    assert!(text.contains("api key"), "the title should be drawn");
    assert!(text.contains(MASK), "the value should be masked");
    assert!(!text.contains("hunter2"));
    assert!(!text.contains('2'));
    assert!(text.contains(CURSOR), "the cursor should still be drawn");
}

#[test]
fn a_select_draws_its_items_and_the_query() {
    let mut app = windowed();
    app.open_select(
        String::from("pick one"),
        vec![String::from("alpha"), String::from("beta")],
    );
    typing(&mut app, "al");
    let text = screen(&app, 40, 12);
    assert!(text.contains("pick one"), "title missing");
    assert!(text.contains("alpha"), "the matching item is missing");
    assert!(!text.contains("beta"), "a filtered item is still drawn");
    assert!(
        text.contains(&format!("al{CURSOR}")),
        "the query and cursor are missing"
    );
}

#[test]
fn a_picker_draws_its_counts_and_preview() {
    let mut app = windowed();
    app.open_pick(
        String::from("files"),
        vec![String::from("one"), String::from("two")],
        false,
    );
    typing(&mut app, "on");
    app.set_preview(vec![String::from("a preview line")]);
    let text = screen(&app, 60, 20);
    assert!(text.contains("files"), "title missing");
    assert!(text.contains("1/2"), "the match counts are missing");
    assert!(text.contains("a preview line"), "the preview is missing");
}

#[test]
fn a_long_message_wraps_to_the_width() {
    let mut app = windowed();
    let words = "wrap ".repeat(40);
    app.conversation().borrow_mut().push(StoredMessage {
        id: MessageId::new(),
        seq: 0,
        time_created: now_millis(),
        message: Message::User { text: words },
    });
    app.reveal_all();
    let width = 30;
    let text = screen(&app, width, 20);
    let rows: Vec<String> = text
        .as_str()
        .chars()
        .collect::<Vec<_>>()
        .chunks(usize::from(width))
        .map(|row| row.iter().collect::<String>().trim_end().to_string())
        .collect();
    let wrapped = rows.iter().filter(|row| row.contains("wrap")).count();
    assert!(wrapped > 1, "a long message should take more than one row");
    assert!(
        rows.iter()
            .all(|row| row.chars().count() <= usize::from(width)),
        "a row should never run past the width"
    );
}

#[test]
fn a_tall_confirm_keeps_its_choices_on_screen_and_scrolls_its_body() {
    let mut app = windowed();
    let body = (1..=200)
        .map(|n| format!("content line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    app.open_confirm(String::from("Write big.txt?"), body);
    let top = screen(&app, 60, 20);
    assert!(top.contains("content line 1 "));
    assert!(top.contains("of 200, scroll for more"));
    assert!(top.contains("1. Yes, proceed"));
    assert!(top.contains("2. No, and tell uji"));
    app.handle_key(Chord::plain(Key::End));
    let bottom = screen(&app, 60, 20);
    assert!(bottom.contains("content line 200"));
    assert!(!bottom.contains("content line 1 "));
    assert!(bottom.contains("1. Yes, proceed"));
}
