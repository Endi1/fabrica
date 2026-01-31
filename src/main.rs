use langrust::client::Model;
use langrust::{GeminiApiModel, GeminiModel, Message, client::FunctionCall};
use std::error::Error;
use std::io::Write;
use std::process::ExitCode;
use std::{env, io};

mod tools;

pub struct Conversation {
    pub contents: Vec<Message>,
}

impl Conversation {
    pub fn add_function_call_message(&mut self, function_call: &FunctionCall) {
        self.contents
            .push(Message::function_call(function_call.clone()));
    }

    pub fn add_message(&mut self, assistant_message: Message) {
        self.contents.push(assistant_message);
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
    print!("> ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input)
}

async fn do_main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Get API key from environment variable
    let api_key = env::var("GEMINI_KEY").expect("GEMINI_KEY environment variable not set");

    // Create a Gemini client with default settings (Gemini 2.5 Flash)
    let client = GeminiApiModel {
        client: reqwest::Client::new(),
        api_key: api_key,
        model: GeminiModel::Gemini25Flash,
    };
    conversation_loop(&client).await?;

    Ok(())
}

async fn conversation_loop(client: &GeminiApiModel) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut state = Conversation { contents: vec![] };
    // let tool_registry = get_filesystem_registry();
    loop {
        let system_prompt = "You are a helpful coding assistant that has access to the file contents of the project the user is working on";
        let user_message_content = get_input()?;

        if user_message_content == "/exit\n" {
            break;
        }

        let user_message = Message::user(user_message_content);
        _ = state.add_message(user_message);

        let mut request_builder = client.new_request();
        let request = request_builder
            .with_system(system_prompt.to_string())
            .with_messages(state.contents.clone());

        let response = request.completion().await?;

        println!("{}", response.completion);

        _ = state.add_message(Message::model(response.completion));
    }
    Ok(())
}
