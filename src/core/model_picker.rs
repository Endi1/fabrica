use crate::core::config::config;
use langrust::client::Model;
use langrust::{
    ClaudeApiModel, ClaudeModel, GeminiApiModel, GeminiModel, GeminiVertexModel, OpenAiApiModel,
    OpenAiModel,
};
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
    pub id: &'static str,
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
        id: "gemini-2.5-flash",
        provider: Provider::Gemini(GeminiModel::Gemini25Flash),
    },
    ModelChoice {
        label: "Google Gemini API  — gemini-3.1-pro",
        id: "gemini-3.1-pro",
        provider: Provider::Gemini(GeminiModel::Gemini31Pro),
    },
    ModelChoice {
        label: "Google Gemini API  — gemini-3-flash",
        id: "gemini-3-flash",
        provider: Provider::Gemini(GeminiModel::Gemini3Flash),
    },
    ModelChoice {
        label: "Google Gemini API  — gemini-3.1-flash-lite",
        id: "gemini-3.1-flash-lite",
        provider: Provider::Gemini(GeminiModel::Gemini31FlashLite),
    },
    ModelChoice {
        label: "Google Vertex AI   — gemini-2.5-flash",
        id: "vertex-gemini-2.5-flash",
        provider: Provider::GeminiVertex(GeminiModel::Gemini25Flash),
    },
    ModelChoice {
        label: "Google Vertex AI  — gemini-3.1-pro",
        id: "vertex-gemini-3.1-pro",
        provider: Provider::Gemini(GeminiModel::Gemini31Pro),
    },
    ModelChoice {
        label: "Google Vertex AI  — gemini-3-flash",
        id: "vertex-gemini-3-flash",
        provider: Provider::Gemini(GeminiModel::Gemini3Flash),
    },
    ModelChoice {
        label: "Google Vertex AI  — gemini-3.1-flash-lite",
        id: "vertex-gemini-3.1-flash-lite",
        provider: Provider::Gemini(GeminiModel::Gemini31FlashLite),
    },
    ModelChoice {
        label: "Anthropic Claude   — claude-sonnet-4-5",
        id: "claude-sonnet-4-5",
        provider: Provider::Claude(ClaudeModel::Sonnet4_5),
    },
    ModelChoice {
        label: "Anthropic Claude   — claude-opus-4-6",
        id: "claude-opus-4-6",
        provider: Provider::Claude(ClaudeModel::Opus4_6),
    },
    ModelChoice {
        label: "Anthropic Claude   — claude-opus-4-7",
        id: "claude-opus-4-7",
        provider: Provider::Claude(ClaudeModel::Opus4_7),
    },
    ModelChoice {
        label: "Openai   — gpt-5.4",
        id: "gpt-5.4",
        provider: Provider::OpenAi(OpenAiModel::Gpt5_4),
    },
    ModelChoice {
        label: "Openai   — gpt-5.4-mini",
        id: "gpt-5.4-mini",
        provider: Provider::OpenAi(OpenAiModel::Gpt5_4Mini),
    },
    ModelChoice {
        label: "Openai   — gpt-5.4-nano",
        id: "gpt-5.4-nano",
        provider: Provider::OpenAi(OpenAiModel::Gpt5_4Nano),
    },
    ModelChoice {
        label: "Openai   — gpt-5.5",
        id: "gpt-5.5",
        provider: Provider::OpenAi(OpenAiModel::Gpt5_5),
    },
    ModelChoice {
        label: "Openai   — gpt-5.3-codex",
        id: "gpt-5.3-codex",
        provider: Provider::OpenAi(OpenAiModel::Gpt5_3Codex),
    },
];

/// Resolve a credential, preferring the env var, then the config file value.
fn resolve_credential(
    env_var: &str,
    config_value: Option<&str>,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    if let Some(v) = config_value
        && !v.is_empty()
    {
        return Ok(v.to_string());
    }
    Err(format!(
        "{} not set (provide via environment variable or fabrica config file)",
        env_var
    )
    .into())
}

fn build(provider: &Provider) -> BuildResult {
    let client = reqwest::Client::new();
    let cfg = config();
    Ok(match provider {
        Provider::Gemini(model) => Box::new(GeminiApiModel {
            client,
            api_key: resolve_credential("GEMINI_KEY", cfg.api_keys.gemini.as_deref())?,
            model: model.clone(),
        }),
        Provider::GeminiVertex(model) => Box::new(GeminiVertexModel {
            client,
            project_name: resolve_credential("GCP_PROJECT", cfg.api_keys.gcp_project.as_deref())?,
            model: model.clone(),
        }),
        Provider::Claude(model) => Box::new(ClaudeApiModel {
            client,
            api_key: resolve_credential("ANTHROPIC_KEY", cfg.api_keys.anthropic.as_deref())?,
            model: model.clone(),
        }),
        Provider::OpenAi(model) => Box::new(OpenAiApiModel {
            client,
            api_key: resolve_credential("OPENAI_KEY", cfg.api_keys.openai.as_deref())?,
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

/// Build a model by its string id (e.g. "claude-opus-4-7").
pub fn build_by_id(model_id: &str) -> BuildResult {
    let choice = CHOICES
        .iter()
        .find(|c| c.id == model_id)
        .ok_or_else(|| format!("unknown model id: {model_id}"))?;
    build(&choice.provider)
}
