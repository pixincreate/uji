use std::cell::RefCell;
use std::collections::VecDeque;
use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use calloop::channel::Sender;
use uji::runtime::{Frontend, Runtime, RuntimeError};
use uji_core::session::model::Message;
use uji_core::session::store::{SessionStorage, Setting};
use uji_core::storage::sqlite::SqliteStorage;
use uji_ui::app::{App, Mode};
use uji_ui::input::Input;
use uji_ui::keymap::{Chord, Key};
use uji_ui::model::RunState;

pub const ALLOW_ALL: &str = r#"
local allow = { default = "allow" }
uji.tool.policy({ default = "allow", read_file = allow, edit_file = allow, write_file = allow, run_command = allow })
"#;

pub const SUBMIT: &str = r#"uji.schedule(function() uji.session.submit("go") end)"#;

pub enum Until {
    TurnFinished,
    Title(&'static str),
}

static SANDBOXES: AtomicUsize = AtomicUsize::new(0);

enum Command {
    Key(char),
    Finish,
}

struct Headless {
    commands: mpsc::Sender<Command>,
    waiting: Option<Receiver<Command>>,
    answers: VecDeque<char>,
    answered: bool,
    worked: bool,
    until: Until,
    deadline: Duration,
}

impl Frontend for Headless {
    fn start(&mut self, events: Sender<Input>) -> io::Result<()> {
        let Some(commands) = self.waiting.take() else {
            return Ok(());
        };
        let until = Instant::now() + self.deadline;
        std::thread::spawn(move || {
            while let Ok(Command::Key(key)) =
                commands.recv_timeout(until.saturating_duration_since(Instant::now()))
            {
                let _ = events.send(Input::Key(Chord::plain(Key::Char(key))));
            }
        });
        Ok(())
    }

    fn draw(&mut self, app: &App) -> io::Result<()> {
        if !matches!(app.mode(), Mode::Confirm { .. }) {
            self.answered = false;
        } else if !self.answered {
            self.answered = true;
            if let Some(key) = self.answers.pop_front() {
                let _ = self.commands.send(Command::Key(key));
            }
        }
        let state = app.state().borrow().run_state();
        self.worked |= state == RunState::Working;
        let done = match self.until {
            Until::TurnFinished => self.worked && state == RunState::Idle,
            Until::Title(title) => app.conversation().borrow().info().title == title,
        };
        if done {
            let _ = self.commands.send(Command::Finish);
        }
        Ok(())
    }

    fn suspend(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn resume(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn stop(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct Sandbox {
    root: PathBuf,
    history: RefCell<Vec<Message>>,
}

impl Sandbox {
    pub fn new(name: &str) -> io::Result<Self> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "uji-test-{name}-{}-{stamp}-{}",
            std::process::id(),
            SANDBOXES.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(root.join("cfg"))?;
        std::fs::create_dir_all(root.join("work"))?;
        Ok(Self {
            root,
            history: RefCell::default(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn work(&self) -> PathBuf {
        self.root.join("work")
    }

    pub fn file(&self, relative: &str, content: impl AsRef<[u8]>) -> io::Result<()> {
        let path = self.work().join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)
    }

    pub fn remember(&self, message: Message) {
        self.history.borrow_mut().push(message);
    }

    pub fn config(&self, lua: &str) -> io::Result<()> {
        std::fs::write(self.root.join("cfg/init.lua"), lua)
    }

    pub fn boot(&self) -> Result<Runtime, RuntimeError> {
        Runtime::boot_in(Some(self.root.join("cfg")))
    }

    pub fn run(
        &self,
        until: Until,
        answers: &[char],
        deadline: Duration,
    ) -> Result<Vec<Message>, Box<dyn Error>> {
        let runtime = self.boot()?;
        let notices = runtime.notices();
        if !notices.is_empty() {
            return Err(format!("boot notices: {notices:?}").into());
        }
        let db = self.root.join("uji.db");
        let mut storage = SqliteStorage::open(db.clone())?;
        storage.set_setting(&Setting::Provider, "test")?;
        storage.set_setting(&Setting::Model, "m")?;
        let mut session = storage.create_session("test")?;
        session.directory = self.work().display().to_string();
        let id = session.id;
        for message in self.history.take() {
            storage.append_message(&id, &message)?;
        }
        let (commands, waiting) = mpsc::channel();
        let frontend = Headless {
            commands,
            waiting: Some(waiting),
            answers: answers.iter().copied().collect(),
            answered: false,
            worked: false,
            until,
            deadline,
        };
        runtime.run_with(&session, Box::new(storage), frontend)?;
        let mut storage = SqliteStorage::open(db)?;
        Ok(storage
            .messages(&id)?
            .into_iter()
            .map(|stored| stored.message)
            .collect())
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub fn provider(url: &str, context: u64) -> String {
    format!(
        r#"uji.provider.add({{ id = "test", name = "Test", wire = "openai-chat", base_url = "{url}/v1", models = {{ {{ id = "m", context = {context}, output = 1000 }} }} }})"#
    )
}

pub fn tool_results(messages: &[Message]) -> Vec<String> {
    messages
        .iter()
        .filter_map(|message| match message {
            Message::Tool { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect()
}
