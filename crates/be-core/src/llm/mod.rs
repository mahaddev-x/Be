pub mod openai;
pub mod anthropic;
pub mod google;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::time::Duration;

use crate::config;

/// Request sent to an LLM provider
pub struct LlmRequest<'a> {
    pub model:       &'a str,
    pub system:      &'a str,
    pub user:        &'a str,
    pub temperature: f32,
    pub max_tokens:  u32,
    pub timeout:     Duration,
}

/// Response from an LLM provider
#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content:       String,
    pub tokens_input:  u32,
    pub tokens_output: u32,
    pub cost_usd:      f64,
}

/// Unified LLM provider trait
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, req: &LlmRequest<'_>) -> Result<LlmResponse>;
    /// Returns (input_cost_per_million, output_cost_per_million) in USD
    fn cost_per_million_tokens(&self, model: &str) -> (f64, f64);
}

/// Route a model string (e.g. "groq/llama-3.1-8b-instant") to the correct provider and call it
pub async fn complete(req: LlmRequest<'_>) -> Result<LlmResponse> {
    let full_model = req.model;
    let (provider_name, model_name) = full_model
        .split_once('/')
        .ok_or_else(|| anyhow!(
            "Invalid model format '{}'. Use: provider/model-name\n\
             Examples: groq/llama-3.1-8b-instant, ollama/qwen2.5:3b, anthropic/claude-haiku-4-5",
            full_model
        ))?;

    let config = config::load()?;

    let provider: Box<dyn LlmProvider> = match provider_name {
        "groq" => Box::new(openai::OpenAiProvider::new(
            "https://api.groq.com/openai/v1".to_string(),
            config.groq_api_key()?,
        )),
        "ollama" => Box::new(openai::OpenAiProvider::new(
            format!("{}/v1", config.ollama_url().trim_end_matches('/')),
            String::new(),
        )),
        "openrouter" => Box::new(openai::OpenAiProvider::new(
            "https://openrouter.ai/api/v1".to_string(),
            config.openrouter_api_key()?,
        )),
        "together" => Box::new(openai::OpenAiProvider::new(
            "https://api.together.xyz/v1".to_string(),
            config.together_api_key()?,
        )),
        "deepseek" => Box::new(openai::OpenAiProvider::new(
            "https://api.deepseek.com/v1".to_string(),
            config.deepseek_api_key()?,
        )),
        "openai" => Box::new(openai::OpenAiProvider::new(
            "https://api.openai.com/v1".to_string(),
            config.openai_api_key()?,
        )),
        "anthropic" => Box::new(anthropic::AnthropicProvider::new(
            config.anthropic_api_key()?,
        )),
        "google" => Box::new(google::GoogleProvider::new(
            config.google_api_key()?,
        )),
        other => return Err(anyhow!(
            "Unknown provider: '{}'. Supported: groq, ollama, openrouter, together, deepseek, openai, anthropic, google",
            other
        )),
    };

    provider.complete(&LlmRequest {
        model: model_name,
        system: req.system,
        user: req.user,
        temperature: req.temperature,
        max_tokens: req.max_tokens,
        timeout: req.timeout,
    }).await
}
