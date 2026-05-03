//! ACP server — a bare JSON-RPC 2.0 server over stdio implementing the
//! Agent Client Protocol (<https://agentclientprotocol.com>).
//!
//! Reads newline-delimited JSON-RPC messages from **stdin** and writes
//! responses / notifications to **stdout**.

use std::collections::HashMap;
use std::sync::Arc;

use langrust::Message;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

use crate::core::agent::{Agent, AgentEvent};
use crate::core::model_picker::BoxedModel;

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 types
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

#[derive(Serialize)]
struct JsonRpcNotification {
    jsonrpc: &'static str,
    method: &'static str,
    params: Value,
}

impl JsonRpcResponse {
    fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }
    fn error(id: Value, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Session state
// ---------------------------------------------------------------------------

struct SessionState {
    agent: Agent,
}

type Sessions = Arc<Mutex<HashMap<String, SessionState>>>;

// ---------------------------------------------------------------------------
// Writer helper – serialises a value and writes it as a single line to stdout
// ---------------------------------------------------------------------------

async fn write_msg(
    stdout: &Mutex<tokio::io::Stdout>,
    msg: &impl Serialize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut line = serde_json::to_string(msg)?;
    line.push('\n');
    let mut out = stdout.lock().await;
    out.write_all(line.as_bytes()).await?;
    out.flush().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run the ACP JSON-RPC server over stdio.
pub async fn run_acp_server(
    system_prompt: String,
    model_factory: Arc<
        dyn Fn() -> Result<BoxedModel, Box<dyn std::error::Error + Send + Sync>> + Send + Sync,
    >,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let sessions: Sessions = Arc::new(Mutex::new(HashMap::new()));
    let stdout = Arc::new(Mutex::new(tokio::io::stdout()));

    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();

    while let Some(line) = lines.next_line().await? {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse::error(Value::Null, -32700, format!("Parse error: {e}"));
                write_msg(&stdout, &resp).await?;
                continue;
            }
        };

        // Dispatch by method
        match req.method.as_str() {
            "initialize" => {
                let resp = handle_initialize(&req);
                write_msg(&stdout, &resp).await?;
            }
            "session/new" => {
                let resp =
                    handle_session_new(&req, &sessions, &system_prompt, &model_factory).await;
                write_msg(&stdout, &resp).await?;
            }
            "session/prompt" => {
                handle_session_prompt(&req, &sessions, &stdout).await?;
            }
            _ => {
                if let Some(id) = &req.id {
                    let resp = JsonRpcResponse::error(
                        id.clone(),
                        -32601,
                        format!("Method not found: {}", req.method),
                    );
                    write_msg(&stdout, &resp).await?;
                }
                // notifications (no id) for unknown methods are silently ignored
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Method handlers
// ---------------------------------------------------------------------------

fn handle_initialize(req: &JsonRpcRequest) -> JsonRpcResponse {
    let id = req.id.clone().unwrap_or(Value::Null);

    // Echo back the protocolVersion the client sent (or default to "1")
    let protocol_version = req
        .params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or("1")
        .to_string();

    JsonRpcResponse::success(
        id,
        serde_json::json!({
            "protocolVersion": protocol_version,
            "agentCapabilities": {},
            "agentInfo": {
                "name": "fabrica",
                "version": env!("CARGO_PKG_VERSION"),
            }
        }),
    )
}

async fn handle_session_new(
    req: &JsonRpcRequest,
    sessions: &Sessions,
    system_prompt: &str,
    model_factory: &Arc<
        dyn Fn() -> Result<BoxedModel, Box<dyn std::error::Error + Send + Sync>> + Send + Sync,
    >,
) -> JsonRpcResponse {
    let id = req.id.clone().unwrap_or(Value::Null);

    let model = match model_factory() {
        Ok(m) => m,
        Err(e) => {
            return JsonRpcResponse::error(id, -32000, format!("Failed to create model: {e}"));
        }
    };

    let session_id = uuid::Uuid::new_v4().to_string();
    let registry = crate::tools::get_filesystem_registry();
    let agent = Agent::new(system_prompt.to_string(), registry, model);

    sessions
        .lock()
        .await
        .insert(session_id.clone(), SessionState { agent });

    JsonRpcResponse::success(id, serde_json::json!({ "sessionId": session_id }))
}

async fn handle_session_prompt(
    req: &JsonRpcRequest,
    sessions: &Sessions,
    stdout: &Arc<Mutex<tokio::io::Stdout>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let id = req.id.clone().unwrap_or(Value::Null);

    let session_id = req
        .params
        .get("sessionId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    // Collect text from prompt content blocks
    let user_text: String = req
        .params
        .get("prompt")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| {
                    if b.get("type").and_then(Value::as_str) == Some("text") {
                        b.get("text").and_then(Value::as_str).map(String::from)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();

    if user_text.is_empty() {
        let resp = JsonRpcResponse::error(id, -32602, "Empty or missing prompt text");
        write_msg(stdout, &resp).await?;
        return Ok(());
    }

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<AgentEvent>();

    // Spawn agent work — the Mutex is held only for the duration of the turn
    let sessions_clone = sessions.clone();
    let agent_handle = tokio::spawn(async move {
        let mut guard = sessions_clone.lock().await;
        let session = guard
            .get_mut(&session_id)
            .ok_or_else(|| format!("unknown session: {session_id}"))?;

        session
            .agent
            .send_user_message(Message::user(user_text), event_tx)
            .await
            .map_err(|e| format!("agent error: {e}"))
    });

    // Stream events as session/update notifications
    let raw_sid = req.params.get("sessionId").cloned().unwrap_or(Value::Null);

    while let Some(event) = event_rx.recv().await {
        match event {
            AgentEvent::TextDelta(text) => {
                let notif = JsonRpcNotification {
                    jsonrpc: "2.0",
                    method: "session/update",
                    params: serde_json::json!({
                        "sessionId": raw_sid,
                        "update": {
                            "sessionUpdate": "agent_message_chunk",
                            "content": {
                                "type": "text",
                                "text": text
                            }
                        }
                    }),
                };
                write_msg(stdout, &notif).await?;
            }
            AgentEvent::ToolCall { name, args } => {
                let tool_call_id = uuid::Uuid::new_v4().to_string();
                let notif = JsonRpcNotification {
                    jsonrpc: "2.0",
                    method: "session/update",
                    params: serde_json::json!({
                        "sessionId": raw_sid,
                        "update": {
                            "sessionUpdate": "tool_call",
                            "toolCallId": tool_call_id,
                            "title": format!("Tool: {name}"),
                            "status": "in_progress",
                            "rawInput": args
                        }
                    }),
                };
                write_msg(stdout, &notif).await?;
            }
            AgentEvent::ToolResult { result: _ } => {
                // Optionally surface as a tool_call_update; for now, silent.
            }
            AgentEvent::ToolError { error } => {
                let notif = JsonRpcNotification {
                    jsonrpc: "2.0",
                    method: "session/update",
                    params: serde_json::json!({
                        "sessionId": raw_sid,
                        "update": {
                            "sessionUpdate": "agent_thought_chunk",
                            "content": {
                                "type": "text",
                                "text": format!("Tool error: {error}")
                            }
                        }
                    }),
                };
                write_msg(stdout, &notif).await?;
            }
            AgentEvent::StreamError(err) => {
                let notif = JsonRpcNotification {
                    jsonrpc: "2.0",
                    method: "session/update",
                    params: serde_json::json!({
                        "sessionId": raw_sid,
                        "update": {
                            "sessionUpdate": "agent_thought_chunk",
                            "content": {
                                "type": "text",
                                "text": format!("Stream error: {err}")
                            }
                        }
                    }),
                };
                write_msg(stdout, &notif).await?;
            }
            AgentEvent::TextDone(()) => {}
        }
    }

    // Wait for the spawned task to finish
    let _ = agent_handle.await;

    // Final response
    let resp = JsonRpcResponse::success(id, serde_json::json!({ "stopReason": "end_turn" }));
    write_msg(stdout, &resp).await?;

    Ok(())
}
