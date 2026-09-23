//! The core: talking to models, gating tools, and persisting sessions.
//!
//! Knows nothing about terminals or Lua, so it can be embedded on its own.
//! Holds the model plumbing ([`llm`]), tool policy ([`tools`]), confined
//! file access ([`fs`]), session storage ([`session`], [`storage`]), and
//! credentials ([`auth`]).

pub mod auth;
pub mod config;
pub mod credential;
pub mod fs;
pub mod llm;
pub mod process;
pub mod session;
pub mod storage;
pub mod tools;
