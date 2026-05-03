use std::process::ExitCode;
use std::sync::Arc;

mod acp;
mod core;
mod tools;
mod tui;

use crate::core::agent::Agent;
use crate::core::{default_model, default_model_label, get_system_prompt};
use crate::tools::get_filesystem_registry;

#[tokio::main]
async fn main() -> ExitCode {
    match do_main().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}

async fn do_main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && args[1] == "serve" {
        // ACP server mode: JSON-RPC 2.0 over stdio
        let registry = get_filesystem_registry();
        let system_prompt = get_system_prompt(&registry);
        let model_factory: Arc<
            dyn Fn() -> Result<
                    crate::core::model_picker::BoxedModel,
                    Box<dyn std::error::Error + Send + Sync>,
                > + Send
                + Sync,
        > = Arc::new(default_model);

        acp::run_acp_server(system_prompt, model_factory).await
    } else {
        // Default: interactive TUI mode
        let registry = get_filesystem_registry();
        let system_prompt = get_system_prompt(&registry);
        let model = default_model()?;
        let agent = Agent::new(system_prompt, registry, model);

        tui::run(agent, default_model_label().to_string()).await
    }
}
