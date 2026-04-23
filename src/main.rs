use std::process::ExitCode;

mod core;
mod tools;
mod tui;

#[tokio::main]
async fn main() -> ExitCode {
    match tui::run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}
