use crate::cmd::{Action, Compact, EffortPick, Help, Login, Models, Reload, Sync};

pub(crate) struct Builtin {
    pub(crate) name: &'static str,
    pub(crate) desc: &'static str,
    make: fn() -> Box<dyn Action>,
}

pub(crate) const BUILTINS: &[Builtin] = &[
    Builtin {
        name: "login",
        desc: "configure provider and auth",
        make: || Box::new(Login::default()),
    },
    Builtin {
        name: "models",
        desc: "pick the default model",
        make: || Box::new(Models::default()),
    },
    Builtin {
        name: "reload",
        desc: "redraw the UI from config",
        make: || Box::new(Reload),
    },
    Builtin {
        name: "effort",
        desc: "how hard the model should think",
        make: || Box::new(EffortPick),
    },
    Builtin {
        name: "compact",
        desc: "summarise earlier messages to free context",
        make: || Box::new(Compact),
    },
    Builtin {
        name: "sync",
        desc: "update installed packs",
        make: || Box::new(Sync),
    },
    Builtin {
        name: "help",
        desc: "list commands",
        make: || Box::new(Help),
    },
];

pub(crate) fn build(name: &str) -> Option<Box<dyn Action>> {
    let found = BUILTINS.iter().find(|builtin| builtin.name == name)?;
    Some((found.make)())
}
