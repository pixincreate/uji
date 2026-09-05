//! The `uji` Lua API: functions exposed to config files and plugins.
//!
//! The built-in functions are registered from a static descriptor table in
//! [`functions`] and delegate to the canonical [`crate::api`] layer;
//! [`convert`] holds the Lua-value conversions.

pub(crate) mod convert;
pub(crate) mod functions;
