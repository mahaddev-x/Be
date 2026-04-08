use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{LlmProvider, LlmRequest, LlmResponse};

pub struct OpenAiProvider {
    base_url: String,
    api_key: String,
    client: Client,
}

impl OpenAiProvider {
    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            base_url,
            api_key,
            client: Client::new(),
        }
    }
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Deserialize)]
struct ChatMessageResponse {
    content: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn complete(&self, req: &LlmRequest<'_>) -> Result<LlmResponse> {
        let mut messages = Vec::new();
        if !req.system.is_empty() {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: req.system.to_string(),
            });
        }
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: req.user.to_string(),
        });

        let body = ChatRequest {
            model: req.model.to_string(),
            messages,
            temperature: req.temperature,
            max_tokens: req.max_tokens,
        };

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let mut request = self.client
            .post(&url)
            .timeout(req.timeout)
            .header("Content-Type", "application/json")
            .json(&body);

        if !self.api_key.is_empty() {
            request = request.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let response = request.send().await
            .map_err(|e| anyhow!("HTTP request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("API error {}: {}", status, body));
        }

        let chat_resp: ChatResponse = response.json().await
            .map_err(|e| anyhow!("Failed to parse API response: {}", e))?;

        let content = chat_resp.choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .unwrap_or_default();

        let (tokens_input, tokens_output) = chat_resp.usage
            .map(|u| (u.prompt_tokens.unwrap_or(0), u.completion_tokens.unwrap_or(0)))
            .unwrap_or((0, 0));

        let (in_cost, out_cost) = self.cost_per_million_tokens(req.model);
        let cost_usd = (tokens_input as f64 * in_cost + tokens_output as f64 * out_cost) / 1_000_000.0;

        Ok(LlmResponse {
            content,
            tokens_input,
            tokens_output,
            cost_usd,
        })
    }

    fn cost_per_million_tokens(&self, model: &str) -> (f64, f64) {
        match model {
            "llama-3.1-8b-instant"    => (0.05, 0.08),
            "llama-3.3-70b-versatile" => (0.59, 0.79),
            "gemma2-9b-it"            => (0.20, 0.20),
            "deepseek-v3"             => (0.01, 0.014),
            "gpt-4o"                  => (5.0, 15.0),
            "gpt-4o-mini"             => (0.15, 0.60),
            _                         => (0.0, 0.0), // local or unknown
        }
    }
}
