use std::{env, fs, io::Error, path::Path};

use crate::tools::{
    CurrentPathResult, DirectoryContentsResult, DirectoryPath, ReadFileContents,
    ReadFileContentsResult,
};

pub fn get_directory_contents(path: DirectoryPath) -> Result<DirectoryContentsResult, Error> {
    let entries = fs::read_dir(path.path)?;
    let mut contents = Vec::new();
    for entry in entries {
        let entry = entry?;
        contents.push(entry.file_name().to_string_lossy().to_string());
    }

    Ok(DirectoryContentsResult { contents })
}

pub fn get_current_path() -> Result<CurrentPathResult, Error> {
    let path = env::current_dir();
    let path_str = path?
        .to_str()
        .ok_or(Error::new(std::io::ErrorKind::NotFound, "Path not found"))?
        .to_string();
    Ok(CurrentPathResult { path: path_str })
}

pub fn read_file_contents(args: ReadFileContents) -> Result<ReadFileContentsResult, Error> {
    let file_path = Path::new(&args.directory).join(&args.filename);
    let contents = fs::read_to_string(file_path)?;

    Ok(ReadFileContentsResult {
        file_contents: contents,
    })
}
