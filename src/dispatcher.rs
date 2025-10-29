use std::{pin::Pin, ptr::copy_nonoverlapping, time::Duration};

use gemini_rust::{
    CacheBuilder, Content, ContentBuilder, FunctionCall, FunctionDeclaration, Gemini,
    GenerationResponse, Message, Role, Tool,
};

use crate::tools::{get_current_path, get_directory_contents, CurrentPathResult, DirectoryContents, DirectoryPath};

pub struct Conversation {
    pub contents: Vec<Message>,
}

pub fn get_tools() -> Tool {
    let get_current_path_tool =
        FunctionDeclaration::new("get_current_path", "Get the current directory path", None)
            .with_response::<CurrentPathResult>();
    let get_directory_contents_tool = FunctionDeclaration::new("get_directory_contents", "Get all the file and folder names for the current directory", None).with_parameters::<DirectoryPath>().with_response::<DirectoryContents>();
    return Tool::with_functions(vec![get_current_path_tool, get_directory_contents_tool]);
}

// TODO Rename this
pub fn not_a_dispatch(
    response: &GenerationResponse,
    conversation: Conversation,
    client: Gemini,
) -> Pin<Box<dyn Future<Output = ()> + '_>> {
    Box::pin(async move {
        let function_calls = response.function_calls();
        let function_call = function_calls.first();
        match function_call {
            Some(fc) => {
                println!("tool called {}", fc.name);
                let conversation = dispatch(fc, conversation);
                match conversation {
                    Ok(convo) => {
                        let content_builder = conversation_to_content_builder(&client, &convo);
                        let response = content_builder.with_tool(get_tools()).execute().await;
                        match response {
                            Err(err) => println!("{}", err),
                            Ok(res) => not_a_dispatch(&res, convo, client).await,
                        }
                    }
                    Err(err) => println!("{}", err),
                }
            }
            None => println!("direct response received {}", response.text()),
        }
    })
}

pub fn dispatch(
    function_call: &&FunctionCall,
    mut conversation: Conversation,
) -> Result<Conversation, String> {
    match function_call.name.as_str() {
        "get_current_path" => {
            let model_message = Message::model(serde_json::to_string(function_call).unwrap());
            conversation.contents.push(model_message);
            let tool_response = get_current_path().unwrap();
            conversation.contents.push(Message::function(
                "get_current_path",
                serde_json::to_value(tool_response).unwrap(),
            ));
            return Ok(conversation);
        }
        "get_directory_contents" => {
            let model_message = Message::model(serde_json::to_string(function_call).unwrap());
            conversation.contents.push(model_message);
            let directory_path: DirectoryPath = serde_json::from_value(function_call.args.clone()).unwrap();
            let tool_response = get_directory_contents(directory_path).unwrap();
            conversation.contents.push(Message::function("get_directory_contents", serde_json::to_value(tool_response).unwrap()));
            return Ok(conversation);
        },
        _ => Err("unknown function name".to_string()), // TODO figure out how to handle this
    }
}

fn conversation_to_content_builder(client: &Gemini, conversation: &Conversation) -> ContentBuilder {
    let mut content_builder = client.generate_content();

    for content in &conversation.contents {
        content_builder = content_builder.with_message(content.clone())
    }

    return content_builder;
}
