use std::pin::Pin;

use gemini_rust::{
    ContentBuilder, FunctionCall, FunctionDeclaration, Gemini, GenerationResponse, Message, Tool,
};

use crate::tools::{
    CurrentPathResult, DirectoryContents, DirectoryPath, ReadFileContents, ReadFileContentsResult,
    get_current_path, get_directory_contents, read_file_contents,
};

pub struct Conversation {
    pub contents: Vec<Message>,
}

pub fn get_tools() -> Tool {
    let get_current_path_tool =
        FunctionDeclaration::new("get_current_path", "Get the current directory path", None)
            .with_response::<CurrentPathResult>();
    let get_directory_contents_tool = FunctionDeclaration::new(
        "get_directory_contents",
        "Get all the file and folder names for the current directory",
        None,
    )
    .with_parameters::<DirectoryPath>()
    .with_response::<DirectoryContents>();
    let read_file_contents_tool = FunctionDeclaration::new(
        "read_file_contents",
        "Reads the file contents for a given file found inside a given path",
        None,
    )
    .with_parameters::<ReadFileContents>()
    .with_response::<ReadFileContentsResult>();
    Tool::with_functions(vec![
        get_current_path_tool,
        get_directory_contents_tool,
        read_file_contents_tool,
    ])
}

// TODO Rename this
pub fn not_a_dispatch<'a>(
    response: &'a GenerationResponse,
    conversation: &'a mut Conversation,
    client: Gemini,
) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
    Box::pin(async {
        let function_calls = response.function_calls();
        let function_call = function_calls.first();
        match function_call {
            Some(fc) => {
                println!("tool called {}", fc.name);
                let dispatch_result = dispatch(fc, conversation);
                match dispatch_result {
                    Ok(_) => {
                        let content_builder =
                            conversation_to_content_builder(&client, conversation);
                        let response = content_builder.with_tool(get_tools()).execute().await;
                        match response {
                            Err(err) => println!("{}", err),
                            Ok(res) => not_a_dispatch(&res, conversation, client).await,
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
    conversation: &mut Conversation,
) -> Result<(), String> {
    match function_call.name.as_str() {
        "get_current_path" => {
            let model_message = Message::model(serde_json::to_string(function_call).unwrap());
            conversation.contents.push(model_message);
            let tool_response = get_current_path().unwrap();
            conversation.contents.push(Message::function(
                "get_current_path",
                serde_json::to_value(tool_response).unwrap(),
            ));
            Ok(())
        }
        "get_directory_contents" => {
            let model_message = Message::model(serde_json::to_string(function_call).unwrap());
            conversation.contents.push(model_message);
            let directory_path: DirectoryPath =
                serde_json::from_value(function_call.args.clone()).unwrap();
            let tool_response = get_directory_contents(directory_path).unwrap();
            conversation.contents.push(Message::function(
                "get_directory_contents",
                serde_json::to_value(tool_response).unwrap(),
            ));
            Ok(())
        }
        "read_file_contents" => {
            let model_message = Message::model(serde_json::to_string(function_call).unwrap());
            conversation.contents.push(model_message);

            let read_file_location: ReadFileContents =
                serde_json::from_value(function_call.args.clone()).unwrap();
            let tool_reponse = read_file_contents(read_file_location).unwrap();
            println!("{}", serde_json::to_value(&tool_reponse).unwrap());
            conversation.contents.push(Message::function(
                "read_file_contents",
                serde_json::to_value(tool_reponse).unwrap(),
            ));

            Ok(())
        }
        _ => Err("unknown function name".to_string()), // TODO figure out how to handle this
    }
}

fn conversation_to_content_builder(client: &Gemini, conversation: &Conversation) -> ContentBuilder {
    let mut content_builder = client.generate_content();

    for content in &conversation.contents {
        content_builder = content_builder.with_message(content.clone())
    }

    content_builder
}
