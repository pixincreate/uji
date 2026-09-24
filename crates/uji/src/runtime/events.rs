use mlua::{Lua, LuaSerdeExt, Table, Value};
use serde::Serialize;

use crate::api::Api;
use uji_core::session::id::SessionId;
use uji_core::session::model::ToolCall;

pub trait Event: Serialize {
    const NAME: &'static str;
}

pub trait Hook: Event {
    const FIELD: &'static str;
}

pub(crate) fn payload<E: Event>(lua: &Lua, event: &E) -> mlua::Result<Table> {
    match lua.to_value(event)? {
        Value::Table(table) => Ok(table),
        _ => lua.create_table(),
    }
}

pub(crate) fn fold<H: Hook>(lua: &Lua, api: &Api, hook: &H, value: &str) -> mlua::Result<String> {
    let payload = payload(lua, hook)?;
    payload.set(H::FIELD, value)?;
    api.fold(H::NAME, &payload, H::FIELD);
    payload.get::<String>(H::FIELD)
}

#[derive(Serialize)]
pub struct SessionCreated {
    pub session_id: SessionId,
}

impl Event for SessionCreated {
    const NAME: &'static str = "session_created";
}

#[derive(Serialize)]
pub struct SessionResumed {
    pub session_id: SessionId,
}

impl Event for SessionResumed {
    const NAME: &'static str = "session_resumed";
}

#[derive(Serialize)]
pub struct SessionTitled<'a> {
    pub title: &'a str,
}

impl Event for SessionTitled<'_> {
    const NAME: &'static str = "session_titled";
}

#[derive(Serialize)]
pub struct SessionCompacted {
    pub count: usize,
}

impl Event for SessionCompacted {
    const NAME: &'static str = "session_compacted";
}

#[derive(Serialize)]
pub struct MessageSubmitted<'a> {
    pub text: &'a str,
}

impl Event for MessageSubmitted<'_> {
    const NAME: &'static str = "message_submitted";
}

#[derive(Serialize)]
pub struct MessageAppended<'a> {
    #[serde(rename = "type")]
    pub kind: &'a str,
    pub text: &'a str,
}

impl Event for MessageAppended<'_> {
    const NAME: &'static str = "message_appended";
}

#[derive(Serialize)]
pub struct QueueChanged {
    pub count: usize,
}

impl Event for QueueChanged {
    const NAME: &'static str = "queue_changed";
}

#[derive(Serialize)]
pub struct ShellStarted<'a> {
    pub command: &'a str,
}

impl Event for ShellStarted<'_> {
    const NAME: &'static str = "shell_started";
}

#[derive(Serialize)]
pub struct ShellFinished<'a> {
    pub command: &'a str,
    pub code: i32,
}

impl Event for ShellFinished<'_> {
    const NAME: &'static str = "shell_finished";
}

#[derive(Serialize)]
pub struct ToolStarted<'a> {
    pub name: &'a str,
}

impl Event for ToolStarted<'_> {
    const NAME: &'static str = "tool_started";
}

#[derive(Serialize)]
pub struct TurnFinished;

impl Event for TurnFinished {
    const NAME: &'static str = "turn_finished";
}

#[derive(Serialize)]
pub struct ModelChanged<'a> {
    pub provider: &'a str,
    pub model: &'a str,
}

impl Event for ModelChanged<'_> {
    const NAME: &'static str = "model_changed";
}

#[derive(Serialize)]
pub struct StatusChanged;

impl Event for StatusChanged {
    const NAME: &'static str = "status_changed";
}

#[derive(Serialize)]
pub struct LoaderTicked;

impl Event for LoaderTicked {
    const NAME: &'static str = "loader_ticked";
}

#[derive(Serialize)]
pub struct BeforeQuit;

impl Event for BeforeQuit {
    const NAME: &'static str = "before_quit";
}

#[derive(Serialize)]
pub struct BeforeTurn<'a> {
    pub text: &'a str,
}

impl Event for BeforeTurn<'_> {
    const NAME: &'static str = "before_turn";
}

impl Hook for BeforeTurn<'_> {
    const FIELD: &'static str = "system";
}

#[derive(Serialize)]
pub struct BeforeTool<'a> {
    pub name: &'a str,
    pub arguments: &'a Table,
}

impl Event for BeforeTool<'_> {
    const NAME: &'static str = "before_tool";
}

#[derive(Serialize)]
pub struct AfterTool<'a> {
    pub name: &'a str,
}

impl Event for AfterTool<'_> {
    const NAME: &'static str = "after_tool";
}

impl Hook for AfterTool<'_> {
    const FIELD: &'static str = "content";
}

#[derive(Serialize)]
pub struct RenderMessage<'a> {
    #[serde(rename = "type")]
    pub kind: &'a str,
    pub text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<&'a str>,
    #[serde(skip_serializing_if = "<[ToolCall]>::is_empty")]
    pub tool_calls: &'a [ToolCall],
}

impl Event for RenderMessage<'_> {
    const NAME: &'static str = "render_message";
}
