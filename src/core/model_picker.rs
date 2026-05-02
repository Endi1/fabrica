use langrust::client::Model;
use langrust::{
    ClaudeApiModel, ClaudeModel, GeminiApiModel, GeminiModel, GeminiVertexModel, OpenAiApiModel,
    OpenAiModel,
};
use std::env;
use std::error::Error;

pub type BoxedModel = Box<dyn Model + Send + Sync>;
type BuildResult = Result<BoxedModel, Box<dyn Error + Send + Sync>>;

enum Provider {
    Gemini(GeminiModel),
    GeminiVertex(GeminiModel),
    Claude(ClaudeModel),
    OpenAi(OpenAiModel),
}

pub struct ModelChoice {
    pub label: &'static str,
    provider: Provider,
}

pub fn model_choices() -> &'static [ModelChoice] {
    CHOICES
}

pub fn default_choice_index() -> usize {
    DEFAULT_CHOICE_INDEX
}

pub fn build_choice(index: usize) -> BuildResult {
    let choice = CHOICES
        .get(index)
        .ok_or_else(|| format!("invalid model index {}", index))?;
    build(&choice.provider)
}

const DEFAULT_CHOICE_INDEX: usize = 10; // Anthropic Claude — claude-opus-4-7

const CHOICES: &[ModelChoice] = &[
    ModelChoice {
        label: "Google Gemini API  — gemini-2.5-flash",
        provider: Provider::Gemini(GeminiModel::Gemini25Flash),
    },
    ModelChoice {
        label: "Google Gemini API  — gemini-3.1-pro",
        provider: Provider::Gemini(GeminiModel::Gemini31Pro),
    },
    ModelChoice {
        label: "Google Gemini API  — gemini-3-flash",
        provider: Provider::Gemini(GeminiModel::Gemini3Flash),
    },
    ModelChoice {
        label: "Google Gemini API  — gemini-3.1-flash-lite",
        provider: Provider::Gemini(GeminiModel::Gemini31FlashLite),
    },
    ModelChoice {
        label: "Google Vertex AI   — gemini-2.5-flash",
        provider: Provider::GeminiVertex(GeminiModel::Gemini25Flash),
    },
    ModelChoice {
        label: "Google Vertex AI  — gemini-3.1-pro",
        provider: Provider::Gemini(GeminiModel::Gemini31Pro),
    },
    ModelChoice {
        label: "Google Vertex AI  — gemini-3-flash",
        provider: Provider::Gemini(GeminiModel::Gemini3Flash),
    },
    ModelChoice {
        label: "Google Vertex AI  — gemini-3.1-flash-lite",
        provider: Provider::Gemini(GeminiModel::Gemini31FlashLite),
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
    ModelChoice {
        label: "Openai   — gpt-5.4",
        provider: Provider::OpenAi(OpenAiModel::Gpt5_4),
    },
    ModelChoice {
        label: "Openai   — gpt-5.4-mini",
        provider: Provider::OpenAi(OpenAiModel::Gpt5_4Mini),
    },
    ModelChoice {
        label: "Openai   — gpt-5.4-nano",
        provider: Provider::OpenAi(OpenAiModel::Gpt5_4Nano),
    },
    ModelChoice {
        label: "Openai   — gpt-5.5",
        provider: Provider::OpenAi(OpenAiModel::Gpt5_5),
    },
    ModelChoice {
        label: "Openai   — gpt-5.3-codex",
        provider: Provider::OpenAi(OpenAiModel::Gpt5_3Codex),
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
            project_name: env_required("GCP_PROJECT")?,
            model: model.clone(),
        }),
        Provider::Claude(model) => Box::new(ClaudeApiModel {
            client,
            api_key: env_required("ANTHROPIC_KEY")?,
            model: model.clone(),
        }),
        Provider::OpenAi(model) => Box::new(OpenAiApiModel {
            client,
            api_key: env_required("OPENAI_KEY")?,
            model: model.clone(),
        }),
    })
}

pub fn default_model() -> BuildResult {
    let choice = &CHOICES[DEFAULT_CHOICE_INDEX];
    build(&choice.provider)
}

pub fn default_model_label() -> &'static str {
    CHOICES[DEFAULT_CHOICE_INDEX].label
}

// The CLI-style picker was replaced by the ratatui-based picker in tui::app.
// model_choices() / build_choice() / default_choice_index() expose the data.
