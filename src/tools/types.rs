use core::fmt;
use std::{any::TypeId, error::Error};

use gemini_rust::FunctionDeclaration;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
struct NotImplementedError {
    message: String,
}

impl fmt::Display for NotImplementedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error {}", self.message)
    }
}

impl std::error::Error for NotImplementedError {}

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

pub struct MyTool<'a, A: JsonSchema + Serialize, R: JsonSchema + for<'de> Deserialize<'de>> {
    pub name: &'a str,
    pub description: &'a str,
    execution: fn(arg: A) -> Result<R, Box<dyn Error>>, // pub declaration: fn() -> FunctionDeclaration,
}

pub trait WithFunctionDeclaration {
    fn get_declaration(&self) -> FunctionDeclaration;
}

impl<'a, A: JsonSchema + Serialize + 'static, R: JsonSchema + for<'de> Deserialize<'de> + Serialize>
    WithFunctionDeclaration for MyTool<'a, A, R>
{
    fn get_declaration(&self) -> FunctionDeclaration {
        let mut function_declaration =
            FunctionDeclaration::new(self.name, self.description, None).with_response::<R>();
        if TypeId::of::<A>() != TypeId::of::<()>() {
            function_declaration = function_declaration.with_parameters::<A>()
        }
        function_declaration
    }
}

impl<'a, A: JsonSchema + Serialize, R: JsonSchema + for<'de> Deserialize<'de> + Serialize>
    MyTool<'a, A, R>
{
    pub fn new(name: &'a str, description: &'a str) -> Self {
        Self {
            name,
            description,
            execution: |_| {
                Err(Box::new(NotImplementedError {
                    message: "Not implemented".to_string(),
                }))
            },
        }
    }

    pub fn with_execution(self, execution: fn(arg: A) -> Result<R, Box<dyn Error>>) -> Self {
        Self {
            name: self.name,
            description: self.description,
            execution,
        }
    }

    pub fn run(self, arg: A) -> Result<R, Box<dyn Error>> {
        (self.execution)(arg)
    }
}
