use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Bee {
    pub name: String,
    pub version: String,
    pub description: String,
    pub model: String,
    pub system_prompt: String,
    pub user_prompt_template: String,
    pub input_vars: Vec<InputVar>,
    pub output_schema: serde_json::Value,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    #[serde(default = "default_max_parallel")]
    pub max_parallel: usize,
    #[serde(default)]
    pub retry: RetryConfig,
}

fn default_temperature() -> f32 { 0.1 }
fn default_max_tokens() -> u32 { 512 }
fn default_timeout() -> u64 { 30 }
fn default_max_parallel() -> usize { 200 }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InputVar {
    pub name: String,
    #[serde(rename = "type", alias = "type_")]
    pub type_: String,
    #[serde(default = "default_true")]
    pub required: bool,
}

fn default_true() -> bool { true }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RetryConfig {
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,
    #[serde(default = "default_backoff")]
    pub backoff_seconds: u64,
}

fn default_max_attempts() -> u32 { 3 }
fn default_backoff() -> u64 { 2 }

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: default_max_attempts(),
            backoff_seconds: default_backoff(),
        }
    }
}

/// Input for a single bee execution — key/value string map
pub type BeeInput = HashMap<String, String>;

impl Bee {
    pub fn from_yaml_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read bee file {:?}: {}", path, e))?;
        let bee: Self = serde_yaml::from_str(&content)
            .map_err(|e| anyhow!("Invalid YAML in {:?}: {}", path, e))?;
        bee.validate()?;
        Ok(bee)
    }

    pub fn from_yaml_str(s: &str) -> Result<Self> {
        let bee: Self = serde_yaml::from_str(s)
            .map_err(|e| anyhow!("Invalid YAML: {}", e))?;
        bee.validate()?;
        Ok(bee)
    }

    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("Bee name cannot be empty"));
        }
        if self.model.is_empty() {
            return Err(anyhow!("Bee model cannot be empty"));
        }
        if !self.model.contains('/') {
            return Err(anyhow!(
                "Invalid model format '{}'. Use: provider/model-name (e.g., groq/llama-3.1-8b-instant)",
                self.model
            ));
        }
        if self.tools.len() > 2 {
            return Err(anyhow!("Bee can have at most 2 tools, got {}", self.tools.len()));
        }
        for tool in &self.tools {
            match tool.as_str() {
                "fetch_url" | "read_file" => {}
                other => return Err(anyhow!("Unknown tool: '{}'. Supported: fetch_url, read_file", other)),
            }
        }
        if self.temperature < 0.0 || self.temperature > 2.0 {
            return Err(anyhow!("temperature must be between 0.0 and 2.0"));
        }
        Ok(())
    }

    /// Substitute {{variable_name}} placeholders in the prompt template
    pub fn interpolate_prompt(&self, input: &BeeInput) -> String {
        let mut result = self.user_prompt_template.clone();
        for (key, value) in input {
            result = result.replace(&format!("{{{{{}}}}}", key), value);
        }
        result
    }

    /// Lookup a bee by name from local bees/, user bees, or built-in bees
    pub fn find(name: &str) -> Result<Self> {
        // 1. ./bees/<name>.yaml
        let local = Path::new("bees").join(format!("{}.yaml", name));
        if local.exists() {
            return Self::from_yaml_file(&local);
        }

        // 2. ~/.be/bees/<name>.yaml
        if let Some(home) = dirs::home_dir() {
            let user = home.join(".be").join("bees").join(format!("{}.yaml", name));
            if user.exists() {
                return Self::from_yaml_file(&user);
            }
        }

        // 3. Built-in bees (embedded at compile time)
        if let Some(yaml) = builtin_bee(name) {
            return Self::from_yaml_str(yaml);
        }

        Err(anyhow!(
            "Bee '{}' not found. Check ./bees/, ~/.be/bees/, or run `be bee list`",
            name
        ))
    }

    /// List all available bees from all sources
    pub fn list_all() -> Vec<String> {
        let mut names: Vec<String> = Vec::new();

        // Built-in bees
        for name in BUILTIN_NAMES {
            names.push(name.to_string());
        }

        // ./bees/
        if let Ok(entries) = std::fs::read_dir("bees") {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "yaml").unwrap_or(false) {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if !names.contains(&stem.to_string()) {
                            names.push(stem.to_string());
                        }
                    }
                }
            }
        }

        // ~/.be/bees/
        if let Some(home) = dirs::home_dir() {
            let user_dir = home.join(".be").join("bees");
            if let Ok(entries) = std::fs::read_dir(user_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|e| e == "yaml").unwrap_or(false) {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            if !names.contains(&stem.to_string()) {
                                names.push(stem.to_string());
                            }
                        }
                    }
                }
            }
        }

        names
    }
}

const BUILTIN_NAMES: &[&str] = &[
    "url-scraper",
    "file-reviewer",
    "text-classifier",
    "data-extractor",
    "sentiment-scorer",
];

fn builtin_bee(name: &str) -> Option<&'static str> {
    match name {
        "url-scraper"      => Some(include_str!("../../../bees/url-scraper.yaml")),
        "file-reviewer"    => Some(include_str!("../../../bees/file-reviewer.yaml")),
        "text-classifier"  => Some(include_str!("../../../bees/text-classifier.yaml")),
        "data-extractor"   => Some(include_str!("../../../bees/data-extractor.yaml")),
        "sentiment-scorer" => Some(include_str!("../../../bees/sentiment-scorer.yaml")),
        _ => None,
    }
}
