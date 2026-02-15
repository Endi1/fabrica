use std::{any::TypeId, collections::HashMap, error::Error};

use langrust::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

type ExecuteFn = Box<dyn Fn(Value) -> Result<Value, Box<dyn Error>>>;

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

pub struct ExecutableTool {
    pub declaration: Tool,
    execute_fn: ExecuteFn,
}

impl ExecutableTool {
    pub fn new<A: JsonSchema + DeserializeOwned + 'static, R: JsonSchema + Serialize + 'static>(
        name: &str,
        description: &str,
        execution: fn(A) -> Result<R, Box<dyn Error>>,
    ) -> Self {
        let mut declaration = Tool {
            name: name.to_string(),
            description: description.to_string(),
            parameters: None,
        };
        if TypeId::of::<A>() != TypeId::of::<()>() {
            declaration = declaration
                .clone()
                .with_parameter::<A>()
                .unwrap_or(declaration);
        }

        Self {
            declaration,
            execute_fn: Box::new(move |args: Value| {
                let typed_args: A = serde_json::from_value(args)?;
                let result: R = execution(typed_args)?;
                let json_result = serde_json::to_value(result)?;
                Ok(json_result)
            }),
        }
    }

    pub fn execute(&self, args: Value) -> Result<Value, Box<dyn Error>> {
        (self.execute_fn)(args)
    }
}

pub struct ToolRegistry {
    map: HashMap<String, ExecutableTool>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: ExecutableTool) -> &mut Self {
        self.map.insert(tool.declaration.name.clone(), tool);
        self
    }

    pub fn get(&self, name: &str) -> Option<&ExecutableTool> {
        self.map.get(name)
    }

    pub fn get_tool_declarations(&self) -> Vec<Tool> {
        self.map.values().map(|t| t.declaration.clone()).collect()
    }

    pub fn execute(
        &self,
        tool_name: &str,
        args_value: Value,
    ) -> Result<Value, Box<dyn Error + Send + Sync>> {
        match self.get(tool_name) {
            Some(tool) => match tool.execute(args_value) {
                Ok(result) => Ok(result),
                Err(e) => {
                    let error_msg = format!("Tool execution error: {}", e);
                    Err(error_msg.into())
                }
            },
            None => {
                let error_msg = format!("Unknown tool: {}", tool_name);
                Err(error_msg.into())
            }
        }
    }
}
