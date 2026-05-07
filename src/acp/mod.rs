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
use crate::core::build_by_id;
use crate::core::model_picker::{self, BoxedModel};

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

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct AvailableModel {
    model_id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct SessionSetupModels {
    current_model_id: String,
    available_models: Vec<AvailableModel>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct SessionSetupResult {
    session_id: String,
    models: SessionSetupModels,
    // config_options: Vec<ConfigOption>,
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
            "session/set_model" => {
                let resp = handle_session_set_model(&req, &sessions, &stdout).await;
                write_msg(&stdout, &resp).await?;
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

fn available_models() -> Vec<AvailableModel> {
    model_picker::model_choices()
        .iter()
        .map(|c| AvailableModel {
            model_id: c.id.to_string(),
            name: c.label.to_string(),
            description: None,
        })
        .collect()
}

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
            "agentCapabilities": {
                "setModel": true
            },
            "agentInfo": {
                "name": "fabrica",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "availableModels": available_models()
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

    let session_setup_result = SessionSetupResult {
        session_id: session_id.clone(),
        models: SessionSetupModels {
            current_model_id: agent.get_model().model_name(),
            available_models: available_models(),
        },
    };

    sessions
        .lock()
        .await
        .insert(session_id.clone(), SessionState { agent });

    JsonRpcResponse::success(id, serde_json::json!(session_setup_result))
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

    let mut last_tool_call_id: Option<String> = None;

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
                last_tool_call_id = Some(tool_call_id.clone());
                let notif = JsonRpcNotification {
                    jsonrpc: "2.0",
                    method: "session/update",
                    params: serde_json::json!({
                        "sessionId": raw_sid,
                        "update": {
                            "sessionUpdate": "tool_call",
                            "toolCallId": tool_call_id,
                            "title": &name,
                            "status": "in_progress",
                            "rawInput": args,
                        }
                    }),
                };
                write_msg(stdout, &notif).await?;
            }
            AgentEvent::ToolResult {
                name: _,
                args: _,
                result,
            } => {
                if let Some(ref tcid) = last_tool_call_id {
                    let text = serde_json::to_string_pretty(&result)
                        .unwrap_or_else(|_| result.to_string());
                    let notif = JsonRpcNotification {
                        jsonrpc: "2.0",
                        method: "session/update",
                        params: serde_json::json!({
                            "sessionId": raw_sid,
                            "update": {
                                "sessionUpdate": "tool_call_update",
                                "toolCallId": tcid,
                                "status": "completed",
                                "content": [{
                                    "type": "content",
                                    "content": {
                                        "type": "text",
                                        "text": text,
                                    }
                                }],
                                "rawOutput": result,
                            }
                        }),
                    };
                    write_msg(stdout, &notif).await?;
                }
            }
            AgentEvent::ToolError {
                name: _,
                args: _,
                error,
            } => {
                if let Some(ref tcid) = last_tool_call_id {
                    let notif = JsonRpcNotification {
                        jsonrpc: "2.0",
                        method: "session/update",
                        params: serde_json::json!({
                            "sessionId": raw_sid,
                            "update": {
                                "sessionUpdate": "tool_call_update",
                                "toolCallId": tcid,
                                "status": "failed",
                                "content": [{
                                    "type": "content",
                                    "content": {
                                        "type": "text",
                                        "text": error.clone(),
                                    }
                                }],
                                "rawOutput": { "error": error },
                            }
                        }),
                    };
                    write_msg(stdout, &notif).await?;
                }
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

async fn handle_session_set_model(
    req: &JsonRpcRequest,
    sessions: &Sessions,
    stdout: &Arc<Mutex<tokio::io::Stdout>>,
) -> JsonRpcResponse {
    let id = req.id.clone().unwrap_or(Value::Null);

    let session_id = req
        .params
        .get("sessionId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let model_id = match req.params.get("modelId").and_then(Value::as_str) {
        Some(m) => m.to_string(),
        None => {
            return JsonRpcResponse::error(id, -32602, "Missing required param: modelId");
        }
    };

    // Build the new model
    let new_model = match build_by_id(&model_id) {
        Ok(m) => m,
        Err(e) => {
            return JsonRpcResponse::error(
                id,
                -32000,
                format!("Failed to create model '{model_id}': {e}"),
            );
        }
    };

    // Swap it into the session
    let mut guard = sessions.lock().await;
    let session = match guard.get_mut(&session_id) {
        Some(s) => s,
        None => {
            return JsonRpcResponse::error(id, -32602, format!("Unknown session: {session_id}"));
        }
    };

    session.agent.set_model(new_model);
    drop(guard);

    // Notify the client about the model change
    let notif = JsonRpcNotification {
        jsonrpc: "2.0",
        method: "session/update",
        params: serde_json::json!({
            "sessionId": session_id,
            "update": {
                "sessionUpdate": "config_option_update",
                "configOptions": [{
                    "id": "model",
                    "label": "Model",
                    "value": model_id
                }]
            }
        }),
    };
    let _ = write_msg(stdout, &notif).await;

    JsonRpcResponse::success(
        id,
        serde_json::json!({
            "modelId": model_id,
            "availableModels": available_models()
        }),
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_models() -> SessionSetupModels {
        SessionSetupModels {
            current_model_id: "claude-opus-4-7".to_string(),
            available_models: vec![
                AvailableModel {
                    model_id: "claude-opus-4-7".to_string(),
                    name: "Claude Opus 4.7".to_string(),
                    description: Some("Most capable model".to_string()),
                },
                AvailableModel {
                    model_id: "gpt-5.4".to_string(),
                    name: "GPT-5.4".to_string(),
                    description: None,
                },
            ],
        }
    }

    #[test]
    fn session_setup_result_serializes_camel_case() {
        let result = SessionSetupResult {
            session_id: "abc-123".to_string(),
            models: test_models(),
        };

        let v = serde_json::json!(result);

        // Top-level keys must be camelCase
        assert_eq!(v["sessionId"], "abc-123");
        assert!(v["models"].is_object());
        assert_eq!(v["models"]["currentModelId"], "claude-opus-4-7");
        // snake_case keys must NOT appear
        assert!(v.get("session_id").is_none());
        assert!(v.get("models").unwrap().get("current_model_id").is_none());
    }

    #[test]
    fn session_setup_models_serializes_camel_case() {
        let v = serde_json::json!(test_models());

        assert_eq!(v["currentModelId"], "claude-opus-4-7");
        assert!(v.get("current_model_id").is_none());

        // availableModels array
        let models = v["availableModels"].as_array().unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0]["modelId"], "claude-opus-4-7");
        assert_eq!(models[0]["name"], "Claude Opus 4.7");
        assert_eq!(models[0]["description"], "Most capable model");
        // second model has no description — key should be absent
        assert_eq!(models[1]["modelId"], "gpt-5.4");
        assert!(models[1].get("description").is_none());
    }

    #[test]
    fn session_setup_result_in_json_rpc_response() {
        let setup = SessionSetupResult {
            session_id: "sess-42".to_string(),
            models: test_models(),
        };

        let resp = JsonRpcResponse::success(serde_json::json!(1), serde_json::json!(setup));

        let v: Value = serde_json::to_value(&resp).unwrap();
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["id"], 1);
        assert_eq!(v["result"]["sessionId"], "sess-42");
        assert_eq!(v["result"]["models"]["currentModelId"], "claude-opus-4-7");
        assert!(v["result"]["models"]["availableModels"].is_array());
        assert!(v.get("error").is_none());
    }
}
