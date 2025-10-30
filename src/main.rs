use gemini_rust::{Gemini, Message};
use std::error::Error;
use std::io::Write;
use std::process::ExitCode;
use std::{env, io};

use crate::dispatcher::{Conversation, get_tools, run_agent};

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

fn get_input() -> Result<String, Box<dyn Error>> {
    print!("> ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input)
}

async fn do_main() -> Result<(), Box<dyn std::error::Error>> {
    // Get API key from environment variable
    let api_key = env::var("GEMINI_KEY").expect("GEMINI_KEY environment variable not set");

    // Create a Gemini client with default settings (Gemini 2.5 Flash)
    let client = Gemini::new(api_key)?;
    conversation_loop(client).await?;

    Ok(())
}

async fn conversation_loop(client: Gemini) -> Result<(), Box<dyn Error>> {
    let mut state = Conversation { contents: vec![] };
    loop {
        let system_prompt = "You are a helpful coding assistant that has access to the file contents of the project the user is working on";
        let user_message_content = get_input()?;

        if user_message_content == "/exit\n" {
            break;
        }

        let user_message = Message::user(user_message_content);
        let mut conversation = client.generate_content();
        conversation = conversation
            .with_system_prompt(system_prompt)
            .with_message(user_message.clone())
            .with_tool(get_tools())
            .with_function_calling_mode(gemini_rust::FunctionCallingMode::Any);
        let response = conversation.execute().await?;

        println!("request sent");

        state.contents.push(user_message.clone());
        run_agent(&response, &mut state, client.clone()).await;
        // return Ok(());
    }
    Ok(())
}
