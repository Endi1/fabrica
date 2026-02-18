use std::error::Error;
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::tools::ExecutableTool;

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub struct BashInput {
    /// The bash command to execute
    pub command: String,
    /// Optional timeout in seconds (default: 30)
    pub timeout: Option<u64>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub struct BashOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

pub fn bash() -> ExecutableTool {
    ExecutableTool::new(
        "bash",
        "Execute a bash command and return its stdout, stderr, and exit code. \
         Use this to run shell commands like grep, find, cat, git, cargo, etc. \
         Commands are executed in the current working directory.",
        |arg: BashInput| -> Result<BashOutput, Box<dyn Error>> {
            let _timeout_secs = arg.timeout.unwrap_or(30);

            let output = Command::new("bash").arg("-c").arg(&arg.command).output()?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code();

            Ok(BashOutput {
                stdout,
                stderr,
                exit_code,
            })
        },
    )
}
