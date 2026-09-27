use globset::{GlobBuilder, GlobMatcher};
use mlua::{IntoLuaMulti, Lua, MultiValue};
use regex::Regex;
use uji_macros::{FromLua, function, methods};

pub(crate) enum Matcher {
    Regex(Regex),
    Glob(GlobMatcher),
}

impl Matcher {
    fn search(&self, subject: &str) -> Option<(usize, usize)> {
        match self {
            Self::Regex(regex) => regex
                .find(subject)
                .map(|found| (found.start(), found.end())),
            Self::Glob(glob) => glob.is_match(subject).then_some((0, subject.len())),
        }
    }
}

#[methods]
impl Matcher {
    fn test(&self, subject: &mlua::LuaString) -> bool {
        self.search(&subject.to_string_lossy()).is_some()
    }

    fn find(&self, lua: &Lua, subject: &mlua::LuaString) -> mlua::Result<MultiValue> {
        match self.search(&subject.to_string_lossy()) {
            Some((start, end)) => (start.saturating_add(1), end).into_lua_multi(lua),
            None => Ok(MultiValue::new()),
        }
    }
}

#[derive(FromLua)]
struct GlobOptions {
    #[lua(default)]
    separator: bool,
}

#[function]
fn regex(pattern: &str) -> Result<Matcher, regex::Error> {
    Regex::new(pattern).map(Matcher::Regex)
}

#[function]
fn glob(pattern: &str, opts: &GlobOptions) -> Result<Matcher, globset::Error> {
    GlobBuilder::new(pattern)
        .literal_separator(opts.separator)
        .build()
        .map(|glob| Matcher::Glob(glob.compile_matcher()))
}
