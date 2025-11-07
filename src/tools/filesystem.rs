use std::{env, fs, io::Error, path::Path};

use gemini_rust::{FunctionDeclaration, Tool};

use crate::tools::{
    CurrentPathResult, DirectoryContentsResult, DirectoryPath, MyTool, ReadFileContents,
    ReadFileContentsResult, WithFunctionDeclaration,
};

pub fn get_tools() -> Tool {
    let declarations: Vec<FunctionDeclaration> = vec![
        get_directory_contents().get_declaration(),
        get_current_path().get_declaration(),
        read_file_contents().get_declaration(),
    ];

    Tool::with_functions(declarations)
}

pub fn get_directory_contents() -> MyTool<'static, DirectoryPath, DirectoryContentsResult> {
    MyTool::new(
        "get_directory_contents",
        "Get all the file and folder names for the current directory",
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

pub fn get_current_path() -> MyTool<'static, (), CurrentPathResult> {
    MyTool::new("get_current_path", "Get the current directory path").with_execution(|()| {
        let path = env::current_dir();
        let path_str = path?
            .to_str()
            .ok_or(Error::new(std::io::ErrorKind::NotFound, "Path not found"))?
            .to_string();
        Ok(CurrentPathResult { path: path_str })
    })
}

pub fn read_file_contents() -> MyTool<'static, ReadFileContents, ReadFileContentsResult> {
    MyTool::new(
        "read_file_contents",
        "Reads the file contents for a given file found inside a given path",
    )
    .with_execution(|arg: ReadFileContents| {
        let file_path = Path::new(&arg.directory).join(&arg.filename);
        let contents = fs::read_to_string(file_path)?;

        Ok(ReadFileContentsResult {
            file_contents: contents,
        })
    })
}
