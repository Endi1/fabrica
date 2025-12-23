use core::fmt;
use std::{any::TypeId, collections::HashMap, error::Error};

use gemini_rust::{FunctionDeclaration, Tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

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
pub struct ReadInput {
    pub filepath: String,
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub struct ReadOutput {
    pub file_contents: String,
}

pub struct MyTool<'a, A: JsonSchema + Serialize, R: JsonSchema + for<'de> Deserialize<'de>> {
    pub name: &'a str,
    pub description: &'a str,
    execution: fn(arg: A) -> Result<R, Box<dyn Error>>,
}

pub trait ExecutableTool {
    fn get_name(&self) -> String;
    fn get_declaration(&self) -> FunctionDeclaration;
    fn execute(&self, args: serde_json::Value) -> Result<serde_json::Value, Box<dyn Error>>;
}

impl<
    'a,
    A: JsonSchema + Serialize + DeserializeOwned + 'static,
    R: JsonSchema + for<'de> Deserialize<'de> + Serialize,
> ExecutableTool for MyTool<'a, A, R>
{
    fn get_name(&self) -> String {
        self.name.to_string()
    }
    fn get_declaration(&self) -> FunctionDeclaration {
        let mut function_declaration =
            FunctionDeclaration::new(self.name, self.description, None).with_response::<R>();
        if TypeId::of::<A>() != TypeId::of::<()>() {
            function_declaration = function_declaration.with_parameters::<A>()
        }
        function_declaration
    }
    fn execute(&self, args: Value) -> Result<Value, Box<dyn Error>> {
        let typed_args: A =
            serde_json::from_value(args).map_err(|e| Box::new(e) as Box<dyn Error>)?;

        let result: R = self.run(typed_args)?;

        let json_result =
            serde_json::to_value(result).map_err(|e| Box::new(e) as Box<dyn Error>)?;

        Ok(json_result)
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

    pub fn run(&self, arg: A) -> Result<R, Box<dyn Error>> {
        (self.execution)(arg)
    }
}

pub struct ToolRegistry {
    map: HashMap<String, Box<dyn ExecutableTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn register<
        'a,
        A: JsonSchema + Serialize + DeserializeOwned + 'static,
        R: JsonSchema + for<'de> Deserialize<'de> + Serialize + 'static,
    >(
        &mut self,
        tool: MyTool<'static, A, R>,
    ) -> &Self {
        self.map.insert(tool.get_name().to_string(), Box::new(tool));
        self
    }

    pub fn get(&self, name: String) -> Option<&dyn ExecutableTool> {
        self.map.get(&name).map(|v| &**v)
    }

    pub fn all_tools(&self) -> Vec<&dyn ExecutableTool> {
        self.map.values().map(|t| t.as_ref()).collect()
    }

    pub fn get_gemini_tool(&self) -> Tool {
        let declarations: Vec<FunctionDeclaration> = self
            .all_tools()
            .iter()
            .map(|tool| tool.get_declaration())
            .collect();

        Tool::with_functions(declarations)
    }
}
