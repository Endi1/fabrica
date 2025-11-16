use std::{error::Error, pin::Pin};

use gemini_rust::{ContentBuilder, FunctionCall, Gemini, GenerationResponse, Message};

use crate::tools::{ToolRegistry, filesystem};

pub struct Conversation {
    pub contents: Vec<Message>,
}

impl Conversation {
    fn add_function_call_message(
        &mut self,
        function_call: &&FunctionCall,
    ) -> Result<(), Box<dyn Error>> {
        let parsed_function_call = serde_json::to_string(function_call)?;
        self.contents.push(Message::model(parsed_function_call));
        Ok(())
    }

    fn add_function_call_result<T: serde::Serialize>(
        &mut self,
        name: String,
        value: T,
    ) -> Result<(), Box<dyn Error>> {
        let parsed_value = serde_json::to_value(value)?;
        self.contents.push(Message::function(name, parsed_value));
        Ok(())
    }
}

pub fn run_agent<'a>(
    response: &'a GenerationResponse,
    conversation: &'a mut Conversation,
    client: Gemini,
    tool_registry: &'a ToolRegistry,
) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
    Box::pin(async {
        let function_calls = response.function_calls();
        let function_call = function_calls.first();
        match function_call {
            Some(fc) => {
                println!("tool called {}", fc.name);
                let dispatch_result = dispatch(tool_registry, fc, conversation);
                match dispatch_result {
                    Ok(_) => {
                        let content_builder =
                            conversation_to_content_builder(&client, conversation);

                        let response = content_builder
                            .with_tool(filesystem::get_tools())
                            .with_function_calling_mode(gemini_rust::FunctionCallingMode::Auto)
                            .execute()
                            .await;
                        match response {
                            Err(err) => println!("{}", err),
                            Ok(res) => run_agent(&res, conversation, client, tool_registry).await,
                        }
                    }
                    Err(err) => println!("{}", err),
                }
            }
            None => println!("{}", response.text()),
        }
    })
}

pub fn dispatch(
    tool_registry: &ToolRegistry,
    function_call: &&FunctionCall,
    conversation: &mut Conversation,
) -> Result<(), Box<dyn Error>> {
    conversation.add_function_call_message(function_call)?;
    let function_name = function_call.name.as_str();
    let tool = tool_registry
        .get(function_name.to_string())
        .ok_or(Box::<dyn Error>::from("Unsupported"))?;

    let tool_response = tool.execute(serde_json::from_value(function_call.args.clone())?)?;
    conversation.add_function_call_result(tool.get_name(), tool_response)?;
    Ok(())

    // match function_call.name.as_str() {
    //     "get_current_path" => {
    //         conversation.add_function_call_message(function_call)?;
    //         let tool = get_current_path();
    //         let tool_response = tool.run(())?;
    //         conversation.add_function_call_result("get_current_path".to_string(), tool_response)
    //     }
    //     "ls" => {
    //         conversation.add_function_call_message(function_call)?;
    //         let directory_path: DirectoryPath = serde_json::from_value(function_call.args.clone())?;
    //         let tool = ls();
    //         let tool_response = tool.run(directory_path)?;
    //         conversation.add_function_call_result(tool.name.to_string(), tool_response)
    //     }
    //     "read" => {
    //         conversation.add_function_call_message(function_call)?;
    //         let read_file_location: ReadInput = serde_json::from_value(function_call.args.clone())?;
    //         let tool = read();
    //         let tool_reponse = tool.run(read_file_location)?;
    //         conversation.add_function_call_result(tool.name.to_string(), tool_reponse)
    //     }
    //     _ => Err(Box::<dyn Error>::from("Unsupported")), // TODO figure out how to handle this
    // }
}

pub fn conversation_to_content_builder(
    client: &Gemini,
    conversation: &Conversation,
) -> ContentBuilder {
    let mut content_builder = client.generate_content();

    for content in &conversation.contents {
        content_builder = content_builder.with_message(content.clone())
    }

    content_builder
}
