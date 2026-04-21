use langrust::client::Model;
use langrust::{ClaudeApiModel, ClaudeModel, GeminiApiModel, GeminiModel, GeminiVertexModel};
use std::env;
use std::error::Error;
use std::io::{self, Write};

type BoxedModel = Box<dyn Model + Send + Sync>;
type BuildResult = Result<BoxedModel, Box<dyn Error + Send + Sync>>;

enum Provider {
    Gemini(GeminiModel),
    GeminiVertex(GeminiModel),
    Claude(ClaudeModel),
}

struct ModelChoice {
    label: &'static str,
    provider: Provider,
}

const DEFAULT_CHOICE_INDEX: usize = 4; // Anthropic Claude — claude-opus-4-7

const CHOICES: &[ModelChoice] = &[
    ModelChoice {
        label: "Google Gemini API  — gemini-2.5-flash",
        provider: Provider::Gemini(GeminiModel::Gemini25Flash),
    },
    ModelChoice {
        label: "Google Vertex AI   — gemini-2.5-flash",
        provider: Provider::GeminiVertex(GeminiModel::Gemini25Flash),
    },
    ModelChoice {
        label: "Anthropic Claude   — claude-sonnet-4-5",
        provider: Provider::Claude(ClaudeModel::Sonnet4_5),
    },
    ModelChoice {
        label: "Anthropic Claude   — claude-opus-4-6",
        provider: Provider::Claude(ClaudeModel::Opus4_6),
    },
    ModelChoice {
        label: "Anthropic Claude   — claude-opus-4-7",
        provider: Provider::Claude(ClaudeModel::Opus4_7),
    },
];

fn env_required(name: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    env::var(name).map_err(|_| format!("{} environment variable not set", name).into())
}

fn build(provider: &Provider) -> BuildResult {
    let client = reqwest::Client::new();
    Ok(match provider {
        Provider::Gemini(model) => Box::new(GeminiApiModel {
            client,
            api_key: env_required("GEMINI_KEY")?,
            model: model.clone(),
        }),
        Provider::GeminiVertex(model) => Box::new(GeminiVertexModel {
            client,
            region: env_required("GCP_REGION")?,
            project_name: env_required("GCP_PROJECT")?,
            model: model.clone(),
        }),
        Provider::Claude(model) => Box::new(ClaudeApiModel {
            client,
            api_key: env_required("ANTHROPIC_API_KEY")?,
            model: model.clone(),
        }),
    })
}

pub fn default_model() -> BuildResult {
    let choice = &CHOICES[DEFAULT_CHOICE_INDEX];
    println!("Using default model: {}", choice.label);
    println!("(Type /model at the prompt to switch providers/models.)\n");
    build(&choice.provider)
}

pub fn pick_model() -> BuildResult {
    println!("Select a provider/model:");
    for (i, c) in CHOICES.iter().enumerate() {
        let marker = if i == DEFAULT_CHOICE_INDEX {
            " (default)"
        } else {
            ""
        };
        println!("  {}. {}{}", i + 1, c.label, marker);
    }

    loop {
        print!("\nEnter choice [1-{}] (blank for default): ", CHOICES.len());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim();

        let index = if trimmed.is_empty() {
            Some(DEFAULT_CHOICE_INDEX)
        } else {
            trimmed
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=CHOICES.len()).contains(n))
                .map(|n| n - 1)
        };

        match index {
            Some(i) => {
                let choice = &CHOICES[i];
                println!("Using: {}\n", choice.label);
                return build(&choice.provider);
            }
            None => println!("Invalid selection, please try again."),
        }
    }
}
