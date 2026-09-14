use crate::cmd::{Action, Args, Context, Help, Login, Models, Reload, Sync};

pub(crate) enum Builtin {
    Login(Login),
    Models(Models),
    Reload(Reload),
    Sync(Sync),
    Help(Help),
}

impl Builtin {
    pub(crate) const ALL: &[(&str, &str)] = &[
        ("login", "configure provider and auth"),
        ("models", "pick the default model"),
        ("reload", "redraw the UI from config"),
        ("sync", "update installed packs"),
        ("help", "list commands"),
    ];

    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match name {
            "login" => Some(Self::Login(Login::default())),
            "models" => Some(Self::Models(Models::default())),
            "reload" => Some(Self::Reload(Reload)),
            "sync" => Some(Self::Sync(Sync)),
            "help" => Some(Self::Help(Help)),
            _ => None,
        }
    }

    pub(crate) fn start<C: Context>(&mut self, ctx: &mut C, args: &Args) {
        match self {
            Self::Login(login) => login.start(ctx, args),
            Self::Models(models) => models.start(ctx, args),
            Self::Reload(reload) => reload.start(ctx, args),
            Self::Sync(sync) => sync.start(ctx, args),
            Self::Help(help) => help.start(ctx, args),
        }
    }

    pub(crate) fn on_select<C: Context>(&mut self, ctx: &mut C, item: String) {
        match self {
            Self::Login(login) => login.on_select(ctx, item),
            Self::Models(models) => models.on_select(ctx, item),
            Self::Reload(reload) => reload.on_select(ctx, item),
            Self::Sync(sync) => sync.on_select(ctx, item),
            Self::Help(help) => help.on_select(ctx, item),
        }
    }

    pub(crate) fn on_prompt<C: Context>(&mut self, ctx: &mut C, value: String) {
        match self {
            Self::Login(login) => login.on_prompt(ctx, value),
            Self::Models(models) => models.on_prompt(ctx, value),
            Self::Reload(reload) => reload.on_prompt(ctx, value),
            Self::Sync(sync) => sync.on_prompt(ctx, value),
            Self::Help(help) => help.on_prompt(ctx, value),
        }
    }

    pub(crate) fn on_cancel<C: Context>(&mut self, ctx: &mut C) {
        match self {
            Self::Login(login) => login.on_cancel(ctx),
            Self::Models(models) => models.on_cancel(ctx),
            Self::Reload(reload) => reload.on_cancel(ctx),
            Self::Sync(sync) => sync.on_cancel(ctx),
            Self::Help(help) => help.on_cancel(ctx),
        }
    }
}
