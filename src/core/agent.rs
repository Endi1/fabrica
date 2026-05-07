use futures::StreamExt;
use std::error::Error;

use langrust::{
    Message, StreamEvent,
    client::{Model, ModelRequestBuilder},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{core::model_picker::BoxedModel, tools::ToolRegistry};

#[derive(Debug, Clone)]
pub enum AgentEvent {
    TextDelta(String),
    TextDone(()),
    ToolCall {
        name: String,
        args: serde_json::Value,
    },
    ToolResult {
        #[allow(dead_code)]
        name: String,
        #[allow(dead_code)]
        args: serde_json::Value,
        result: serde_json::Value,
    },
    ToolError {
        #[allow(dead_code)]
        name: String,
        #[allow(dead_code)]
        args: serde_json::Value,
        error: String,
    },
    StreamError(String),
}

pub struct Conversation {
    pub contents: Vec<Message>,
}

impl Conversation {
    pub fn add_message(&mut self, message: Message) {
        self.contents.push(message);
    }

    pub fn new() -> Conversation {
        Conversation { contents: vec![] }
    }
}

pub struct Agent {
    conversation: Conversation,
    system_prompt: String,
    registry: ToolRegistry,
    model: BoxedModel,
}

impl Agent {
    pub fn new(system_prompt: String, registry: ToolRegistry, model: BoxedModel) -> Agent {
        Agent {
            conversation: Conversation::new(),
            system_prompt,
            registry,
            model,
        }
    }

    pub async fn send_user_message(
        &mut self,
        user_message: Message,
        event_channel: UnboundedSender<AgentEvent>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.conversation.add_message(user_message);
        let tool_declarations = self.registry.get_tool_declarations();

        loop {
            let mut request_builder = ModelRequestBuilder::new(self.model.as_ref() as &dyn Model);
            let request = request_builder
                // TODO Make the settings configurable by the user
                .with_settings(langrust::Settings {
                    temperature: None,
                    max_tokens: None,
                    timeout: None,
                    thinking_budget: None,
                })
                .with_system(self.system_prompt.to_string())
                .with_messages(self.conversation.contents.clone())
                .with_tools(tool_declarations.clone());

            let mut stream = request.stream().await?;
            let mut full_response = String::new();
            let mut pending_function_call = None;

            while let Some(event) = stream.next().await {
                match event {
                    StreamEvent::Delta(t) => {
                        full_response.push_str(&t);
                        let _ = event_channel.send(AgentEvent::TextDelta(t));
                    }
                    StreamEvent::Error(e) => {
                        let _ = event_channel.send(AgentEvent::StreamError(e.to_string()));
                        break;
                    }
                    StreamEvent::Usage(_) => continue,
                    StreamEvent::FunctionCall(fc) => {
                        pending_function_call = Some(fc);
                    }
                }
            }

            match pending_function_call {
                Some(fc) => {
                    let args_value = serde_json::to_value(&fc.args)?;
                    let _ = event_channel.send(AgentEvent::ToolCall {
                        name: fc.name.clone(),
                        args: args_value.clone(),
                    });

                    self.conversation
                        .add_message(Message::function_call(fc.clone()));

                    match self.registry.execute(&fc.name, args_value.clone()) {
                        Ok(result) => {
                            let _ = event_channel.send(AgentEvent::ToolResult {
                                name: fc.name.clone(),
                                args: args_value.clone(),
                                result: result.clone(),
                            });
                            self.conversation
                                .add_message(Message::function_result(fc.name, result));
                        }
                        Err(e) => {
                            let error_msg = format!("Tool execution error: {}", e);
                            let _ = event_channel.send(AgentEvent::ToolError {
                                name: fc.name.clone(),
                                args: args_value.clone(),
                                error: error_msg.clone(),
                            });
                            self.conversation.add_message(Message::function_result(
                                fc.name,
                                serde_json::json!({ "error": error_msg }),
                            ));
                        }
                    }
                    continue;
                }
                None => {
                    if !full_response.trim().is_empty() {
                        self.conversation
                            .add_message(Message::model(full_response.clone()));
                    }
                    let _ = event_channel.send(AgentEvent::TextDone(()));
                    break;
                }
            }
        }
        Ok(())
    }

    pub fn set_model(&mut self, model: BoxedModel) {
        self.model = model
    }

    pub fn get_model(&self) -> &BoxedModel {
        &self.model
    }
}
