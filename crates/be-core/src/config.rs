use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub providers: ProvidersConfig,
    #[serde(default)]
    pub defaults: DefaultsConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ProvidersConfig {
    #[serde(default)]
    pub groq_api_key: String,
    #[serde(default)]
    pub anthropic_api_key: String,
    #[serde(default)]
    pub openai_api_key: String,
    #[serde(default)]
    pub openrouter_api_key: String,
    #[serde(default)]
    pub together_api_key: String,
    #[serde(default)]
    pub deepseek_api_key: String,
    #[serde(default)]
    pub google_api_key: String,
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,
}

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DefaultsConfig {
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_parallel")]
    pub parallel: usize,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    #[serde(default = "default_retries")]
    pub max_retries: u32,
}

fn default_model() -> String { "groq/llama-3.1-8b-instant".to_string() }
fn default_parallel() -> usize { 50 }
fn default_timeout() -> u64 { 30 }
fn default_retries() -> u32 { 3 }

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            model: default_model(),
            parallel: default_parallel(),
            timeout_seconds: default_timeout(),
            max_retries: default_retries(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            providers: ProvidersConfig::default(),
            defaults: DefaultsConfig::default(),
        }
    }
}

pub fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".be")
        .join("config.toml")
}

pub fn load() -> Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| anyhow!("Failed to read config {:?}: {}", path, e))?;
    let config: Config = toml::from_str(&content)
        .map_err(|e| anyhow!("Invalid config.toml: {}", e))?;
    Ok(config)
}

pub fn save(config: &Config) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(config)
        .map_err(|e| anyhow!("Failed to serialize config: {}", e))?;
    std::fs::write(&path, content)
        .map_err(|e| anyhow!("Failed to write config {:?}: {}", path, e))?;
    Ok(())
}

impl Config {
    pub fn groq_api_key(&self) -> Result<String> {
        let key = &self.providers.groq_api_key;
        if key.is_empty() {
            return Err(anyhow!(
                "Groq API key not set. Run: be config set providers.groq_api_key <key>"
            ));
        }
        Ok(key.clone())
    }

    pub fn anthropic_api_key(&self) -> Result<String> {
        let key = &self.providers.anthropic_api_key;
        if key.is_empty() {
            return Err(anyhow!(
                "Anthropic API key not set. Run: be config set providers.anthropic_api_key <key>"
            ));
        }
        Ok(key.clone())
    }

    pub fn openai_api_key(&self) -> Result<String> {
        let key = &self.providers.openai_api_key;
        if key.is_empty() {
            return Err(anyhow!(
                "OpenAI API key not set. Run: be config set providers.openai_api_key <key>"
            ));
        }
        Ok(key.clone())
    }

    pub fn openrouter_api_key(&self) -> Result<String> {
        let key = &self.providers.openrouter_api_key;
        if key.is_empty() {
            return Err(anyhow!(
                "OpenRouter API key not set. Run: be config set providers.openrouter_api_key <key>"
            ));
        }
        Ok(key.clone())
    }

    pub fn together_api_key(&self) -> Result<String> {
        let key = &self.providers.together_api_key;
        if key.is_empty() {
            return Err(anyhow!(
                "Together API key not set. Run: be config set providers.together_api_key <key>"
            ));
        }
        Ok(key.clone())
    }

    pub fn deepseek_api_key(&self) -> Result<String> {
        let key = &self.providers.deepseek_api_key;
        if key.is_empty() {
            return Err(anyhow!(
                "DeepSeek API key not set. Run: be config set providers.deepseek_api_key <key>"
            ));
        }
        Ok(key.clone())
    }

    pub fn google_api_key(&self) -> Result<String> {
        let key = &self.providers.google_api_key;
        if key.is_empty() {
            return Err(anyhow!(
                "Google API key not set. Run: be config set providers.google_api_key <key>"
            ));
        }
        Ok(key.clone())
    }

    pub fn ollama_url(&self) -> String {
        self.providers.ollama_url.clone()
    }

    /// Set a dot-notation key, e.g. "providers.groq_api_key"
    pub fn set_key(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "providers.groq_api_key"       => self.providers.groq_api_key = value.to_string(),
            "providers.anthropic_api_key"  => self.providers.anthropic_api_key = value.to_string(),
            "providers.openai_api_key"     => self.providers.openai_api_key = value.to_string(),
            "providers.openrouter_api_key" => self.providers.openrouter_api_key = value.to_string(),
            "providers.together_api_key"   => self.providers.together_api_key = value.to_string(),
            "providers.deepseek_api_key"   => self.providers.deepseek_api_key = value.to_string(),
            "providers.google_api_key"     => self.providers.google_api_key = value.to_string(),
            "providers.ollama_url"         => self.providers.ollama_url = value.to_string(),
            "defaults.model"               => self.defaults.model = value.to_string(),
            "defaults.parallel"            => {
                self.defaults.parallel = value.parse()
                    .map_err(|_| anyhow!("Invalid parallel value: {}", value))?;
            }
            "defaults.timeout_seconds"     => {
                self.defaults.timeout_seconds = value.parse()
                    .map_err(|_| anyhow!("Invalid timeout value: {}", value))?;
            }
            "defaults.max_retries"         => {
                self.defaults.max_retries = value.parse()
                    .map_err(|_| anyhow!("Invalid max_retries value: {}", value))?;
            }
            _ => return Err(anyhow!("Unknown config key: '{}'. Valid keys: providers.groq_api_key, providers.anthropic_api_key, providers.openai_api_key, providers.openrouter_api_key, providers.together_api_key, providers.deepseek_api_key, providers.google_api_key, providers.ollama_url, defaults.model, defaults.parallel, defaults.timeout_seconds, defaults.max_retries", key)),
        }
        Ok(())
    }

    /// Get a dot-notation key value
    pub fn get_key(&self, key: &str) -> Result<String> {
        let val = match key {
            "providers.groq_api_key"       => self.providers.groq_api_key.clone(),
            "providers.anthropic_api_key"  => self.providers.anthropic_api_key.clone(),
            "providers.openai_api_key"     => self.providers.openai_api_key.clone(),
            "providers.openrouter_api_key" => self.providers.openrouter_api_key.clone(),
            "providers.together_api_key"   => self.providers.together_api_key.clone(),
            "providers.deepseek_api_key"   => self.providers.deepseek_api_key.clone(),
            "providers.google_api_key"     => self.providers.google_api_key.clone(),
            "providers.ollama_url"         => self.providers.ollama_url.clone(),
            "defaults.model"               => self.defaults.model.clone(),
            "defaults.parallel"            => self.defaults.parallel.to_string(),
            "defaults.timeout_seconds"     => self.defaults.timeout_seconds.to_string(),
            "defaults.max_retries"         => self.defaults.max_retries.to_string(),
            _ => return Err(anyhow!("Unknown config key: '{}'", key)),
        };
        Ok(val)
    }
}
