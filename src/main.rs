use langrust::Message;
use std::error::Error;
use std::io;
use std::io::Write;
use std::process::ExitCode;
use tokio::sync::mpsc::{self, UnboundedReceiver};

mod core;
mod tools;
use core::{get_system_prompt, pick_model};

use crate::{
    core::agent::{Agent, AgentEvent},
    tools::get_filesystem_registry,
};

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
        let (event_channel_sender, event_channel_receiver) =
            mpsc::unbounded_channel::<AgentEvent>();

        // Run the agent and render its events concurrently on this task.
        let (send_result, _) = tokio::join!(
            agent.send_user_message(user_message, event_channel_sender),
            display_events(event_channel_receiver),
        );
        send_result?;
    }
    Ok(())
}

async fn display_events(mut rx: UnboundedReceiver<AgentEvent>) {
    while let Some(event) = rx.recv().await {
        match event {
            AgentEvent::TextDelta(t) => {
                print!("{}", t);
                io::stdout().flush().ok();
            }
            AgentEvent::TextDone(_) => {
                println!();
            }
            AgentEvent::ToolCall { name, args } => {
                println!(
                    "\n[tool: {}] [args: {}]",
                    name,
                    serde_json::to_string(&args).unwrap_or_default()
                );
            }
            AgentEvent::ToolResult { result } => {
                let result_str =
                    serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
                println!(
                    "[result: {}...]\n",
                    &result_str[..result_str.len().min(200)]
                );
            }
            AgentEvent::ToolError { error } => {
                eprintln!("[error: {}]", error);
            }
            AgentEvent::StreamError(e) => {
                eprintln!("\nStream error: {}", e);
            }
        }
    }
}
