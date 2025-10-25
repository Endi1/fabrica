use gemini_rust::{Content, FunctionDeclaration, Gemini, Message, Role, Tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::env;
use std::process::ExitCode;

mod tools;

/// Basic content generation example - demonstrates the simplest usage of the Gemini API
#[tokio::main]
async fn main() -> ExitCode {
    match do_main().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            println!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}

#[derive(Serialize, Deserialize, JsonSchema)]
struct Weather {
    location: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct WeatherResponse {
    temperature: i32,
    unit: String,
    condition: String,
}

async fn do_main() -> Result<(), Box<dyn std::error::Error>> {
    // Get API key from environment variable
    let api_key = env::var("GEMINI_KEY").expect("GEMINI_KEY environment variable not set");

    // Create a Gemini client with default settings (Gemini 2.5 Flash)
    let client = Gemini::new(api_key)?;

    println!("basic content generation example starting");

    let get_weather = FunctionDeclaration::new(
        "get_weather",
        "Get the current weather for a location",
        None,
    )
    .with_parameters::<Weather>()
    .with_response::<WeatherResponse>();

    let tool = Tool::with_functions(vec![get_weather]);

    let system_prompt = "You are a helpful assistant that checks the weather and responds to the user about anything they ask";
    let user_message = "What can you do?";
    let mut conversation = client.generate_content();
    conversation = conversation
        .with_system_prompt(system_prompt)
        .with_user_message(user_message)
        .with_tool(tool)
        .with_function_calling_mode(gemini_rust::FunctionCallingMode::Any);
    let response = conversation.execute().await?;

    println!("request sent");

    if let Some(function_call) = response.function_calls().first() {
        // Handle different function calls
        match function_call.name.as_str() {
            "get_weather" => {
                println!("parsing function call result");

                let weather: Weather = serde_json::from_value(function_call.args.clone())?;
                let mut conversation = client.generate_content();
                conversation = conversation
                    .with_system_prompt(system_prompt)
                    .with_user_message(user_message);

                // 2. Create model content with function call
                let model_content = Content::function_call((*function_call).clone());

                // Add as model message
                let model_message = Message {
                    content: model_content,
                    role: Role::Model,
                };
                conversation = conversation.with_message(model_message);

                let weather_response = WeatherResponse {
                    temperature: 22,
                    unit: "C".to_string(),
                    condition: "sunny".to_string(),
                };
                println!("weather response is parsed");

                // 3. Add user message with function response
                conversation =
                    conversation.with_function_response("get_weather", weather_response)?;

                // Execute the request
                let final_response = conversation.execute().await?;
                println!("final response: {}", final_response.text());
            }
            _ => println!(
                "function_name = {} unknown function call",
                function_call.name
            ),
        }
    } else {
        println!("no function calls in response");
        println!("direct response received {}", response.text())
    }
    Ok(())
}
