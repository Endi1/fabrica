use std::{env, fs, io::Error, path::Path};

use gemini_rust::{FunctionDeclaration, Tool};
use serde_json::Value;

use crate::tools::{
    CurrentPathResult, DirectoryContentsResult, DirectoryPath, ExecutableTool, MyTool, ReadInput,
    ReadOutput, ToolRegistry,
};

pub fn get_filesystem_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(ls());
    registry.register(get_current_path());
    registry.register(read());
    registry
}

pub fn get_tools() -> Tool {
    let declarations: Vec<FunctionDeclaration> = vec![
        ls().get_declaration(),
        get_current_path().get_declaration(),
        read().get_declaration(),
    ];

    Tool::with_functions(declarations)
}

pub fn ls() -> MyTool<'static, DirectoryPath, DirectoryContentsResult> {
    MyTool::new(
        "ls",
        "Lists files and directories in a given path. The path parameter must be an absolute path, not a relative path.",
    )
    .with_execution(|arg| {
        let entries = fs::read_dir(arg.path)?;
        let mut contents = Vec::new();
        for entry in entries {
            let entry = entry?;
            contents.push(entry.file_name().to_string_lossy().to_string());
        }

        Ok(DirectoryContentsResult { contents })
    })
}

pub fn get_current_path() -> MyTool<'static, Value, CurrentPathResult> {
    MyTool::new("get_current_path", "Get the current directory path").with_execution(|_| {
        let path = env::current_dir();
        let path_str = path?
            .to_str()
            .ok_or(Error::new(std::io::ErrorKind::NotFound, "Path not found"))?
            .to_string();
        Ok(CurrentPathResult { path: path_str })
    })
}

pub fn read() -> MyTool<'static, ReadInput, ReadOutput> {
    MyTool::new(
        "read",
        "Reads a file from the local filesystem. You can access any file directly by using this tool.
Assume this tool is able to read all files on the machine. If the User provides a path to a file assume that path is valid. It is okay to read a file that does not exist; an error will be returned.

Usage:
- The file_path parameter must be an absolute path, not a relative path
- By default, it reads up to 2000 lines starting from the beginning of the file
- You can optionally specify a line offset and limit (especially handy for long files), but it's recommended to read the whole file by not providing these parameters
- Any lines longer than 2000 characters will be truncated
- Results are returned using cat -n format, with line numbers starting at 1
- This tool allows Claude Code to read images (eg PNG, JPG, etc). When reading an image file the contents are presented visually as Claude Code is a multimodal LLM.
- This tool can read PDF files (.pdf). PDFs are processed page by page, extracting both text and visual content for analysis.
- This tool can read Jupyter notebooks (.ipynb files) and returns all cells with their outputs, combining code, text, and visualizations.
- You have the capability to call multiple tools in a single response. It is always better to speculatively read multiple files as a batch that are potentially useful.
- You will regularly be asked to read screenshots. If the user provides a path to a screenshot ALWAYS use this tool to view the file at the path. This tool will work with all temporary file paths like /var/folders/123/abc/T/TemporaryItems/NSIRD_screencaptureui_ZfB1tD/Screenshot.png
- If you read a file that exists but has empty contents you will receive a system reminder warning in place of file contents.
",
    )
    .with_execution(|arg: ReadInput| {
        let file_path = Path::new(&arg.filepath);
        let contents = fs::read_to_string(file_path)?;

        Ok(ReadOutput {
            file_contents: contents,
        })
    })
}
