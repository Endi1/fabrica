// use gemini_rust::FunctionDeclaration;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
// use std::io::Error;

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DirectoryContents {
    path: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub struct CurrentPathResult {
    pub path: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub struct DirectoryContentsResult {
    pub contents: Vec<String>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub struct DirectoryPath {
    pub path: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub struct ReadFileContents {
    pub directory: String,
    pub filename: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub struct ReadFileContentsResult {
    pub file_contents: String,
}

// pub struct MyTool<A: JsonSchema + Serialize, R: JsonSchema + for<'de> Deserialize<'de>> {
//     name: String,
//     description: String,
//     run: fn(arg: A) -> Result<R, Error>,
//     declaration: FunctionDeclaration,
// }
