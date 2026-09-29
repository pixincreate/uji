use std::path::PathBuf;

use mlua::{Function, Lua, Table};
use tokio::runtime::Runtime;

use crate::context::{self, Context};
use crate::net;
use crate::queue::Channel;
use crate::tty::{self, Terminal, Tty};
use crate::vm::{self, Sources};

const MODULE_DIR: &str = "lua";
const SCHEDULER: &str = "uji.kernel.scheduler";

pub struct Options {
    pub sources: Sources,
    pub entry: String,
    pub args: Vec<String>,
    pub terminal: Terminal,
}

pub struct Outcome {
    pub code: u8,
    pub errors: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("the kernel lost its state")]
    Lost,
    #[error("{0}")]
    Lua(#[from] mlua::Error),
}

pub(crate) struct Restart {
    pub(crate) args: Vec<String>,
    pub(crate) roots: Vec<PathBuf>,
    pub(crate) carry: Option<String>,
}

struct Life {
    code: u8,
    errors: Vec<String>,
    restart: Option<Restart>,
    terminal: Option<Tty>,
    channel: Channel,
}

pub fn run(options: Options) -> Outcome {
    drive(options).unwrap_or_else(|err| Outcome {
        code: 1,
        errors: vec![err.to_string()],
    })
}

fn drive(options: Options) -> Result<Outcome, Error> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    drop(runtime.spawn_blocking(net::warm));
    let Options {
        sources,
        entry,
        args,
        terminal,
    } = options;
    let mut next = Restart {
        args,
        roots: Vec::new(),
        carry: None,
    };
    let mut terminal = Some(Tty::Fresh(terminal));
    let mut channel = Channel::open();
    let mut errors = Vec::new();
    let code = loop {
        let life = live(&runtime, (&sources, &entry), next, terminal.take(), channel)?;
        errors.extend(life.errors);
        terminal = life.terminal;
        channel = life.channel;
        match life.restart {
            Some(restart) => next = restart,
            None => break life.code,
        }
    };
    tty::restore();
    drop(terminal);
    runtime.shutdown_background();
    Ok(Outcome { code, errors })
}

fn layers(sources: &Sources, roots: &[PathBuf]) -> Vec<Sources> {
    roots
        .iter()
        .map(|root| Sources::Directory(root.join(MODULE_DIR)))
        .chain(std::iter::once(sources.clone()))
        .collect()
}

fn publish(lua: &Lua, boot: &Restart) -> mlua::Result<()> {
    let os = lua.create_table()?;
    let roots = boot.roots.iter().map(|root| root.display().to_string());
    os.set("roots", lua.create_sequence_from(roots)?)?;
    os.set("carry", boot.carry.clone())?;
    lua.globals().get::<Table>("uji")?.set("os", os)
}

fn live(
    runtime: &Runtime,
    (sources, entry): (&Sources, &str),
    boot: Restart,
    terminal: Option<Tty>,
    channel: Channel,
) -> Result<Life, Error> {
    let layers = layers(sources, &boot.roots);
    context::enter(Context::new(
        runtime.handle().clone(),
        layers.clone(),
        terminal,
        channel,
    ));
    let ran = start(layers, entry, boot);
    let context = context::leave().ok_or(Error::Lost)?;
    ran?;
    Ok(Life {
        code: context.exit.unwrap_or(0),
        errors: context.errors,
        restart: context.restart,
        terminal: context.terminal,
        channel: context.queue.close(),
    })
}

fn start(layers: Vec<Sources>, entry: &str, boot: Restart) -> Result<(), Error> {
    let lua = vm::create(layers)?;
    publish(&lua, &boot)?;
    let main: Function = vm::require(&lua, entry)?;
    let scheduler: Table = vm::require(&lua, SCHEDULER)?;
    let args = lua.create_sequence_from(boot.args)?;
    scheduler.get::<Function>("run")?.call::<()>((main, args))?;
    Ok(())
}
