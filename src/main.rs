use gemini_rust::{Content, FunctionDeclaration, Gemini, Message, Role, Tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::env;
use std::process::ExitCode;

use crate::dispatcher::{Conversation, get_tools, not_a_dispatch};

mod dispatcher;
mod tools;

/// Basic content generation example - demonstrates the simplest usage of the Gemini API
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

#[derive(Serialize, Deserialize, JsonSchema)]
struct Weather {
    location: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct WeatherResponse {
    temperature: i32,
    unit: String,
    condition: String,
}

async fn do_main() -> Result<(), Box<dyn std::error::Error>> {
    // Get API key from environment variable
    let api_key = env::var("GEMINI_KEY").expect("GEMINI_KEY environment variable not set");

    // Create a Gemini client with default settings (Gemini 2.5 Flash)
    let client = Gemini::new(api_key)?;


    let system_prompt = "You are a helpful coding assistant that has access to the file contents of the project the user is working on";
    let user_message = "What is the current path?";
    let mut conversation = client.generate_content();
    conversation = conversation
        .with_system_prompt(system_prompt)
        .with_user_message(user_message)
        .with_tool(get_tools())
        .with_function_calling_mode(gemini_rust::FunctionCallingMode::Any);
    let response = conversation.execute().await?;

    println!("request sent");

    not_a_dispatch(&response, Conversation { contents: vec![] }, client).await;
    Ok(())
}
