//! The `uji` Lua API: functions exposed to config files and plugins.
//!
//! Every callable is a type implementing [`Lua`]: it carries the key it is
//! registered under and knows how to create the underlying
//! [`mlua::Function`] bound to a Lua state. Embedders can implement this
//! trait to extend the API with their own functions.

use mlua::{Function, Lua as LuaState, Table};

pub(crate) mod functions;

/// A single callable exposed to the `uji` Lua table.
pub trait Lua {
    /// Key the function is registered under (e.g. `"open_win"`).
    fn key(&self) -> &'static str;

    /// Human-readable name of the function.
    fn name(&self) -> &'static str;

    /// Create the callable bound to the given Lua state.
    fn create_function(&self, lua: &LuaState) -> mlua::Result<Function>;

    /// Register the function under its key in `table`.
    fn register(&self, lua: &LuaState, table: &Table) -> mlua::Result<()> {
        table.set(self.key(), self.create_function(lua)?)
    }
}
