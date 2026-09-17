use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::oneshot;

use crate::session::model::{Message, ToolCall};
use crate::tools::progress::Progress;
use crate::tools::{Invocation, Tool, ToolRegistry};

use super::cancel::CancelToken;
use super::catalog::Budget;
use super::context;
use super::event::{StreamEvent, ToolDecision};
use super::request::{LlmRequest, LlmResponse, ToolSpec, Usage};
use super::tuning::{Effort, Retention};
use super::{Protocol, RETRY_ATTEMPTS, backoff, clip, summary};

pub const MAX_TOOL_ITERATIONS: usize = 100;

const TOOL_TIMEOUT: Duration = Duration::from_secs(300);

pub struct AgentConfig<'a, P: Protocol + ?Sized> {
    pub client: &'a reqwest::Client,
    pub provider: &'a P,
    pub model: String,
    pub system: Option<String>,
    pub tools: &'a ToolRegistry,
    pub lua_tools: &'a [LuaToolSpec],
    pub cwd: &'a Path,
    pub cancel: CancelToken,
    pub budget: Option<Budget>,
    pub keep_recent: u64,
    pub effort: Effort,
    pub max_output: u32,
    pub cache: Retention,
}

#[derive(Clone)]
pub struct LuaToolSpec {
    pub name: String,
    pub spec: ToolSpec,
    pub subject: String,
}

async fn generate<P: Protocol + ?Sized>(
    config: &AgentConfig<'_, P>,
    request: &LlmRequest<'_>,
    on_event: &mut (dyn FnMut(StreamEvent) + Send),
) -> Option<LlmResponse> {
    let mut attempt = 0;
    loop {
        let mut on_delta = |delta: String| on_event(StreamEvent::Delta(delta));
        let streaming = config.provider.call(config.client, request, &mut on_delta);
        let outcome = tokio::select! {
            result = streaming => result,
            () = config.cancel.cancelled() => {
                on_event(StreamEvent::Cancelled);
                return None;
            }
        };
        let err = match outcome {
            Ok(response) => return Some(response),
            Err(err) => err,
        };
        if attempt >= RETRY_ATTEMPTS || !err.retryable() || config.cancel.is_cancelled() {
            on_event(StreamEvent::Failed(err.to_string()));
            return None;
        }
        let wait = err.retry_after().unwrap_or_else(|| backoff(attempt));
        attempt = attempt.saturating_add(1);
        on_event(StreamEvent::Restarted {
            attempt,
            of: RETRY_ATTEMPTS,
            wait,
        });
        tokio::select! {
            () = tokio::time::sleep(wait) => {}
            () = config.cancel.cancelled() => {
                on_event(StreamEvent::Cancelled);
                return None;
            }
        }
    }
}

async fn compact_turn<P: Protocol + ?Sized>(
    config: &AgentConfig<'_, P>,
    messages: &mut Vec<Message>,
) -> Option<(String, Option<Usage>)> {
    let budget = config.budget?;
    if !budget.overflows(context::estimate_messages(messages)) {
        return None;
    }
    let at = {
        let refs: Vec<&Message> = messages.iter().collect();
        context::cut_index(&refs, config.keep_recent)?
    };
    let carried = messages
        .first()
        .map_or_else(Vec::new, context::previous_files);
    let previous = messages
        .first()
        .and_then(|message| context::previous_summary(message))
        .map(str::to_string);
    let skip = usize::from(previous.is_some());
    let head: Vec<Message> = messages[skip..at].to_vec();
    let mut files = context::files_touched(&head.iter().collect::<Vec<_>>());
    context::merge_files(&mut files, &carried);
    let summarised = {
        let refs: Vec<&Message> = head.iter().collect();
        summary::generate(
            config.client,
            config.provider,
            config.model.clone(),
            &refs,
            previous.as_deref(),
        )
        .await?
    };
    let tail = messages.split_off(at);
    messages.clear();
    messages.push(context::summary_message(&summarised.summary, &files));
    messages.extend(tail);
    Some((summarised.summary, summarised.usage))
}

async fn steer(
    messages: &mut Vec<Message>,
    on_event: &mut (dyn FnMut(StreamEvent) + Send),
) -> bool {
    let (reply, receiver) = oneshot::channel();
    on_event(StreamEvent::SteerRequest { reply });
    let Ok(Some(text)) = receiver.await else {
        return false;
    };
    messages.push(Message::User { text });
    true
}

pub async fn run_agent<P: Protocol + ?Sized>(
    config: &AgentConfig<'_, P>,
    mut messages: Vec<Message>,
    on_event: &mut (dyn FnMut(StreamEvent) + Send),
) {
    let mut tool_specs = config.tools.specs();
    tool_specs.extend(config.lua_tools.iter().map(|tool| tool.spec.clone()));
    let tool_names: Vec<String> = tool_specs.iter().map(|spec| spec.name.clone()).collect();
    for _ in 0..MAX_TOOL_ITERATIONS {
        if config.cancel.is_cancelled() {
            on_event(StreamEvent::Cancelled);
            return;
        }
        if let Some((summary, usage)) = compact_turn(config, &mut messages).await {
            on_event(StreamEvent::Compacted { summary, usage });
        }
        steer(&mut messages, on_event).await;
        let request = LlmRequest {
            model: &config.model,
            system: config.system.as_deref(),
            messages: &messages,
            tools: &tool_specs,
            effort: config.effort,
            max_output: config.max_output,
            cache: config.cache,
        };
        let Some(response) = generate(config, &request, on_event).await else {
            return;
        };
        if let Some(usage) = response.usage {
            on_event(StreamEvent::Usage(usage));
        }
        let text = response.text;
        let reasoning_content = response.reasoning_content;
        let mut tool_calls = response.tool_calls;
        for call in &mut tool_calls {
            if call.id.trim().is_empty() {
                call.id = format!("call_{}", uuid::Uuid::now_v7().simple());
            }
        }
        if tool_calls.is_empty() {
            if steer(&mut messages, on_event).await {
                on_event(StreamEvent::AssistantStep {
                    text: text.clone(),
                    tool_calls: Vec::new(),
                    reasoning_content: reasoning_content.clone(),
                });
                messages.push(Message::Assistant {
                    text,
                    tool_calls: Vec::new(),
                    reasoning_content,
                });
                continue;
            }
            on_event(StreamEvent::Done {
                text,
                reasoning_content,
            });
            return;
        }
        on_event(StreamEvent::AssistantStep {
            text: text.clone(),
            tool_calls: tool_calls.clone(),
            reasoning_content: reasoning_content.clone(),
        });
        messages.push(Message::Assistant {
            text,
            tool_calls: tool_calls.clone(),
            reasoning_content,
        });
        for tool_call in &tool_calls {
            let content = if config.cancel.is_cancelled() {
                String::from("error: interrupted by the user before this tool ran")
            } else {
                tokio::select! {
                    result = execute_tool(config, tool_call, &tool_names, on_event) => result,
                    () = config.cancel.cancelled() => {
                        String::from("error: interrupted by the user while this tool ran")
                    }
                }
            };
            on_event(StreamEvent::ToolResult {
                tool_call_id: tool_call.id.clone(),
                name: tool_call.name.clone(),
                content: content.clone(),
            });
            messages.push(Message::Tool {
                tool_call_id: tool_call.id.clone(),
                name: tool_call.name.clone(),
                content,
            });
        }
        if config.cancel.is_cancelled() {
            on_event(StreamEvent::Cancelled);
            return;
        }
    }
    on_event(StreamEvent::Failed(format!(
        "stopped after {MAX_TOOL_ITERATIONS} tool iterations without a final answer"
    )));
}

fn parse_arguments(raw: &str) -> Result<Value, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(value @ Value::Object(_)) => Ok(value),
        Ok(other) => Err(format!(
            "error: arguments must be a JSON object, got {}. Send a single object matching the tool schema.",
            json_kind(&other)
        )),
        Err(err) => Err(format!(
            "error: arguments are not valid JSON ({err}). Send a single JSON object matching the tool schema, with no markdown fences. Received: {}",
            clip(trimmed, 500)
        )),
    }
}

fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

enum Target<'a> {
    Native(&'a Arc<dyn Tool>),
    Lua(&'a LuaToolSpec),
}

fn find_tool<'a, P: Protocol + ?Sized>(
    config: &'a AgentConfig<'_, P>,
    name: &str,
) -> Option<Target<'a>> {
    if let Some(tool) = config.tools.get(name) {
        return Some(Target::Native(tool));
    }
    config
        .lua_tools
        .iter()
        .find(|tool| tool.name == name)
        .map(Target::Lua)
}

async fn execute_tool<P: Protocol + ?Sized>(
    config: &AgentConfig<'_, P>,
    tool_call: &ToolCall,
    tool_names: &[String],
    on_event: &mut (dyn FnMut(StreamEvent) + Send),
) -> String {
    let Some(target) = find_tool(config, &tool_call.name) else {
        return format!(
            "error: unknown tool `{}`. Available tools: {}.",
            tool_call.name,
            tool_names.join(", ")
        );
    };
    let args = match parse_arguments(&tool_call.arguments) {
        Ok(args) => args,
        Err(message) => return message,
    };
    let subject = match target {
        Target::Native(tool) => tool.subject(&args),
        Target::Lua(tool) => tool.subject.clone(),
    };
    let (reply, receiver) = oneshot::channel();
    on_event(StreamEvent::ToolDecisionRequest {
        tool: tool_call.clone(),
        subject,
        reply,
    });
    let decision = match tokio::time::timeout(TOOL_TIMEOUT, receiver).await {
        Ok(Ok(decision)) => decision,
        Ok(Err(_)) => return String::from("denied: no decision"),
        Err(_) => return String::from("denied: timed out waiting for confirmation"),
    };
    match decision {
        ToolDecision::Deny { reason } => format!("denied: {reason}"),
        ToolDecision::Allow { arguments } => {
            let args = parse_arguments(&arguments).unwrap_or(args);
            match target {
                Target::Native(tool) => {
                    run_native_tool(config, tool.as_ref(), &args, tool_call, on_event).await
                }
                Target::Lua(tool) => run_lua_tool(&tool.name, &args, on_event).await,
            }
        }
    }
}

/// Run a native tool, forwarding whatever it reports while it works.
async fn run_native_tool<P: Protocol + ?Sized>(
    config: &AgentConfig<'_, P>,
    tool: &dyn Tool,
    args: &Value,
    tool_call: &ToolCall,
    on_event: &mut (dyn FnMut(StreamEvent) + Send),
) -> String {
    let (sink, mut updates) = tokio::sync::mpsc::unbounded_channel();
    let progress = Progress::new(sink);
    let call = Invocation::new(config.cwd, &config.cancel, &progress);
    let mut running = std::pin::pin!(tool.run(args, &call));
    loop {
        tokio::select! {
            outcome = &mut running => {
                while let Ok(chunk) = updates.try_recv() {
                    emit_progress(tool_call, chunk, on_event);
                }
                return match outcome {
                    Ok(text) => text,
                    Err(err) => format!("error: {err}"),
                };
            }
            Some(chunk) = updates.recv() => emit_progress(tool_call, chunk, on_event),
        }
    }
}

fn emit_progress(
    tool_call: &ToolCall,
    chunk: String,
    on_event: &mut (dyn FnMut(StreamEvent) + Send),
) {
    on_event(StreamEvent::ToolProgress {
        tool_call_id: tool_call.id.clone(),
        name: tool_call.name.clone(),
        chunk,
    });
}

async fn run_lua_tool(
    name: &str,
    args: &Value,
    on_event: &mut (dyn FnMut(StreamEvent) + Send),
) -> String {
    let (reply, receiver) = oneshot::channel();
    on_event(StreamEvent::RunLuaTool {
        name: name.to_string(),
        arguments: args.to_string(),
        reply,
    });
    match tokio::time::timeout(TOOL_TIMEOUT, receiver).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => String::from("error: lua tool did not respond"),
        Err(_) => String::from("error: lua tool timed out"),
    }
}
