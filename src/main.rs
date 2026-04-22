use langrust::Message;
use std::error::Error;
use std::io;
use std::io::Write;
use std::process::ExitCode;

mod core;
mod tools;
use core::{get_system_prompt, pick_model};

use crate::{core::agent::Agent, tools::get_filesystem_registry};

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
    let registry = get_filesystem_registry();
    let sp = get_system_prompt();
    let mut agent = Agent::new(sp, registry)?;
    conversation_loop(&mut agent).await?;

    Ok(())
}

async fn conversation_loop(agent: &mut Agent) -> Result<(), Box<dyn Error + Send + Sync>> {
    loop {
        let user_message_content = get_input()?;
        let trimmed = user_message_content.trim();

        if trimmed == "/exit" {
            break;
        }

        if trimmed == "/model" {
            match pick_model() {
                Ok(new_client) => {
                    agent.set_model(new_client);
                }
                Err(e) => {
                    eprintln!("Failed to switch model: {}", e);
                }
            }
            continue;
        }

        let user_message = Message::user(user_message_content);
        agent.send_user_message(user_message).await?;
    }
    Ok(())
}
