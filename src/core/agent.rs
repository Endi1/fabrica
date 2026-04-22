use futures::StreamExt;
use std::{
    error::Error,
    io::{self, Write},
};

use langrust::{
    Message, StreamEvent,
    client::{FunctionCall, Model, ModelRequestBuilder},
};

use crate::{
    core::{default_model, model_picker::BoxedModel},
    tools::ToolRegistry,
};

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
    pub fn new(
        system_prompt: String,
        registry: ToolRegistry,
    ) -> Result<Agent, Box<dyn Error + Send + Sync>> {
        return Ok(Agent {
            conversation: Conversation::new(),
            system_prompt,
            registry,
            model: default_model()?,
        });
    }

    pub async fn send_user_message(
        &mut self,
        user_message: Message,
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
            let mut pending_function_call: Option<FunctionCall> = None;

            while let Some(event) = stream.next().await {
                match event {
                    StreamEvent::Delta(t) => {
                        full_response.push_str(&t);
                        print!("{}", t);
                        io::stdout().flush().ok();
                    }
                    StreamEvent::Error(e) => {
                        eprintln!("\nStream error: {}", e);
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
                    println!(
                        "\n[tool: {}] [args: {}]",
                        fc.name,
                        serde_json::to_string(&fc.args).unwrap_or_default()
                    );

                    // Add the model's function call to conversation
                    self.conversation
                        .add_message(Message::function_call(fc.clone()));

                    // Execute the tool
                    let args_value = serde_json::to_value(&fc.args)?;
                    let function_call_result = self.registry.execute(&fc.name, args_value);
                    match function_call_result {
                        Ok(result) => {
                            let result_str = serde_json::to_string_pretty(&result)
                                .unwrap_or_else(|_| result.to_string());
                            println!(
                                "[result: {}...]\n",
                                &result_str[..result_str.len().min(200)]
                            );

                            // Add the function result to conversation — pass Value directly
                            self.conversation
                                .add_message(Message::function_result(fc.name, result));
                        }
                        Err(e) => {
                            let error_msg = format!("Tool execution error: {}", e);
                            eprintln!("[error: {}]", error_msg);
                            self.conversation.add_message(Message::function_result(
                                fc.name,
                                serde_json::json!({ "error": error_msg }),
                            ));
                        }
                    }
                    // Continue the loop to let the model respond with the tool result
                    continue;
                }
                None => {
                    // No function call — model gave a text response
                    if !full_response.is_empty() {
                        self.conversation.add_message(Message::model(full_response));
                    }
                    println!(); // newline after response
                    break;
                }
            }
        }
        Ok(())
    }

    pub fn set_model(&mut self, model: BoxedModel) {
        self.model = model
    }
}
