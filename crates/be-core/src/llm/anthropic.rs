use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{LlmProvider, LlmRequest, LlmResponse};

pub struct AnthropicProvider {
    api_key: String,
    client: Client,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
        }
    }
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<AnthropicMessage>,
    temperature: f32,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    usage: Option<AnthropicUsage>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn complete(&self, req: &LlmRequest<'_>) -> Result<LlmResponse> {
        let body = AnthropicRequest {
            model: req.model.to_string(),
            max_tokens: req.max_tokens,
            system: req.system.to_string(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: req.user.to_string(),
            }],
            temperature: req.temperature,
        };

        let response = self.client
            .post("https://api.anthropic.com/v1/messages")
            .timeout(req.timeout)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow!("HTTP request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Anthropic API error {}: {}", status, body));
        }

        let resp: AnthropicResponse = response.json().await
            .map_err(|e| anyhow!("Failed to parse Anthropic response: {}", e))?;

        let content = resp.content
            .into_iter()
            .find(|c| c.content_type == "text")
            .and_then(|c| c.text)
            .unwrap_or_default();

        let (tokens_input, tokens_output) = resp.usage
            .map(|u| (u.input_tokens.unwrap_or(0), u.output_tokens.unwrap_or(0)))
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
        if model.contains("haiku") {
            (0.25, 1.25)
        } else if model.contains("sonnet") {
            (3.0, 15.0)
        } else if model.contains("opus") {
            (15.0, 75.0)
        } else {
            (3.0, 15.0)
        }
    }
}
