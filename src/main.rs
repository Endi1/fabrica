use futures::StreamExt;
use langrust::StreamEvent;
use langrust::client::Model;
use langrust::{GeminiApiModel, GeminiModel, Message, client::FunctionCall};
use std::error::Error;
use std::io::Write;
use std::process::ExitCode;
use std::{env, io};

mod core;
mod tools;
use core::get_system_prompt;
use tools::{ToolRegistry, get_filesystem_registry};

pub struct Conversation {
    pub contents: Vec<Message>,
}

impl Conversation {
    pub fn add_message(&mut self, message: Message) {
        self.contents.push(message);
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    match do_main().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            println!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}

fn get_input() -> Result<String, Box<dyn Error + Send + Sync>> {
    print!("\n> ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input)
}

async fn do_main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let api_key = env::var("GEMINI_KEY").expect("GEMINI_KEY environment variable not set");

    let client = GeminiApiModel {
        client: reqwest::Client::new(),
        api_key,
        model: GeminiModel::Gemini25Flash,
    };

    let registry = get_filesystem_registry();
    conversation_loop(&client, &registry).await?;

    Ok(())
}

async fn conversation_loop(
    client: &GeminiApiModel,
    registry: &ToolRegistry,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut state = Conversation { contents: vec![] };
    let system_prompt = get_system_prompt();
    let tool_declarations = registry.get_tool_declarations();

    loop {
        let user_message_content = get_input()?;

        if user_message_content.trim() == "/exit" {
            break;
        }

        let user_message = Message::user(user_message_content);
        state.add_message(user_message);

        // Inner loop: keep calling the model until we get a text response (no more tool calls)
        loop {
            let mut request_builder = client.new_request();
            let request = request_builder
                // TODO Make the settings configurable by the user
                .with_settings(langrust::Settings {temperature: Some(0), max_tokens: None, timeout: None, thinking_budget: None})
                .with_system(system_prompt.to_string())
                .with_messages(state.contents.clone())
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
                    state.add_message(Message::function_call(fc.clone()));

                    // Execute the tool
                    let args_value = serde_json::to_value(&fc.args)?;
                    let function_call_result = registry.execute(&fc.name, args_value);
                    match function_call_result {
                        Ok(result) => {
                            let result_str = serde_json::to_string_pretty(&result)
                                .unwrap_or_else(|_| result.to_string());
                            println!(
                                "[result: {}...]\n",
                                &result_str[..result_str.len().min(200)]
                            );

                            // Add the function result to conversation — pass Value directly
                            state.add_message(Message::function_result(fc.name, result));
                        }
                        Err(e) => {
                            let error_msg = format!("Tool execution error: {}", e);
                            eprintln!("[error: {}]", error_msg);
                            state.add_message(Message::function_result(
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
                        state.add_message(Message::model(full_response));
                    }
                    println!(); // newline after response
                    break;
                }
            }
        }
    }
    Ok(())
}
