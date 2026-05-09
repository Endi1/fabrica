use serde::Deserialize;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Fabrica configuration loaded from a TOML file.
///
/// Lookup order:
///   1. `$FABRICA_CONFIG` (if set)
///   2. `$XDG_CONFIG_HOME/fabrica/config.toml` (or `~/.config/fabrica/config.toml`)
///   3. Platform config dir (e.g. `~/Library/Application Support/fabrica/config.toml` on macOS)
///   4. `~/.fabrica.conf`
///
/// API keys missing from the file fall back to environment variables:
///   - `gemini`      <- `GEMINI_API_KEY`, `GEMINI_KEY`
///   - `anthropic`   <- `ANTHROPIC_API_KEY`, `ANTHROPIC_KEY`, `CLAUDE_API_KEY`
///   - `openai`      <- `OPENAI_API_KEY`, `OPENAI_KEY`
///   - `gcp_project` <- `GOOGLE_CLOUD_PROJECT`, `GCP_PROJECT`
///
/// Example:
/// ```toml
/// [api_keys]
/// gemini = "..."
/// anthropic = "..."
/// openai = "..."
/// gcp_project = "my-project"
/// ```
#[derive(Debug, Default, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub api_keys: ApiKeys,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct ApiKeys {
    #[serde(default, alias = "GEMINI_KEY")]
    pub gemini: Option<String>,
    #[serde(default, alias = "ANTHROPIC_KEY", alias = "claude")]
    pub anthropic: Option<String>,
    #[serde(default, alias = "OPENAI_KEY")]
    pub openai: Option<String>,
    #[serde(default, alias = "GCP_PROJECT")]
    pub gcp_project: Option<String>,
}

impl ApiKeys {
    /// Fill in any missing keys from environment variables.
    fn merge_env(&mut self) {
        if self.gemini.is_none() {
            self.gemini = first_env(&["GEMINI_API_KEY", "GEMINI_KEY"]);
        }
        if self.anthropic.is_none() {
            self.anthropic = first_env(&["ANTHROPIC_API_KEY", "ANTHROPIC_KEY", "CLAUDE_API_KEY"]);
        }
        if self.openai.is_none() {
            self.openai = first_env(&["OPENAI_API_KEY", "OPENAI_KEY"]);
        }
        if self.gcp_project.is_none() {
            self.gcp_project = first_env(&["GOOGLE_CLOUD_PROJECT", "GCP_PROJECT"]);
        }
    }
}

fn first_env(names: &[&str]) -> Option<String> {
    for name in names {
        match std::env::var(name) {
            Ok(v) if !v.is_empty() => return Some(v),
            _ => continue,
        }
    }
    None
}

static CONFIG: OnceLock<Config> = OnceLock::new();

/// Returns the global config, loading it on first access.
/// Errors during loading are logged to stderr and an empty config is used.
pub fn config() -> &'static Config {
    CONFIG.get_or_init(|| match load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("warning: failed to load fabrica config: {}", e);
            let mut cfg = Config::default();
            cfg.api_keys.merge_env();
            cfg
        }
    })
}

/// Find the config file path on disk, if any exists.
pub fn config_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("FABRICA_CONFIG") {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Some(p);
        }
    }

    // Honor XDG_CONFIG_HOME explicitly, falling back to ~/.config (even on
    // platforms like macOS where `dirs::config_dir()` returns
    // ~/Library/Application Support).
    let xdg_base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")));
    if let Some(base) = xdg_base {
        let xdg_based = base.join("fabrica").join("config.toml");
        if xdg_based.is_file() {
            return Some(xdg_based);
        }
    }

    // Platform-native config dir as a secondary location (e.g.
    // ~/Library/Application Support/fabrica/config.toml on macOS).
    if let Some(config_dir) = dirs::config_dir() {
        let dir_based = config_dir.join("fabrica").join("config.toml");
        if dir_based.is_file() {
            return Some(dir_based);
        }
    }

    if let Some(home) = dirs::home_dir() {
        let dotfile = home.join(".fabrica.conf");
        if dotfile.is_file() {
            return Some(dotfile);
        }
    }

    None
}

fn load() -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
    let mut cfg = match config_path() {
        Some(path) => {
            let contents = std::fs::read_to_string(&path)
                .map_err(|e| format!("reading {}: {}", path.display(), e))?;
            toml::from_str::<Config>(&contents)
                .map_err(|e| format!("parsing {}: {}", path.display(), e))?
        }
        None => Config::default(),
    };
    cfg.api_keys.merge_env();
    Ok(cfg)
}
