use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{LlmProvider, LlmRequest, LlmResponse};

pub struct GoogleProvider {
    api_key: String,
    client: Client,
}

impl GoogleProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
        }
    }
}

#[derive(Serialize)]
struct GoogleRequest {
    contents: Vec<GoogleContent>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GoogleSystemInstruction>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Serialize)]
struct GoogleSystemInstruction {
    parts: Vec<GooglePart>,
}

#[derive(Serialize)]
struct GoogleContent {
    role: String,
    parts: Vec<GooglePart>,
}

#[derive(Serialize, Deserialize)]
struct GooglePart {
    text: String,
}

#[derive(Serialize)]
struct GenerationConfig {
    temperature: f32,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
}

#[derive(Deserialize)]
struct GoogleResponse {
    candidates: Option<Vec<GoogleCandidate>>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<GoogleUsage>,
}

#[derive(Deserialize)]
struct GoogleCandidate {
    content: Option<GoogleContent2>,
}

#[derive(Deserialize)]
struct GoogleContent2 {
    parts: Option<Vec<GooglePart>>,
}

#[derive(Deserialize)]
struct GoogleUsage {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: Option<u32>,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: Option<u32>,
}

#[async_trait]
impl LlmProvider for GoogleProvider {
    async fn complete(&self, req: &LlmRequest<'_>) -> Result<LlmResponse> {
        let model = req.model;
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, self.api_key
        );

        let system_instruction = if !req.system.is_empty() {
            Some(GoogleSystemInstruction {
                parts: vec![GooglePart { text: req.system.to_string() }],
            })
        } else {
            None
        };

        let body = GoogleRequest {
            system_instruction,
            contents: vec![GoogleContent {
                role: "user".to_string(),
                parts: vec![GooglePart { text: req.user.to_string() }],
            }],
            generation_config: GenerationConfig {
                temperature: req.temperature,
                max_output_tokens: req.max_tokens,
            },
        };

        let response = self.client
            .post(&url)
            .timeout(req.timeout)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow!("HTTP request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Google API error {}: {}", status, body));
        }

        let resp: GoogleResponse = response.json().await
            .map_err(|e| anyhow!("Failed to parse Google response: {}", e))?;

        let content = resp.candidates
            .and_then(|cs| cs.into_iter().next())
            .and_then(|c| c.content)
            .and_then(|c| c.parts)
            .and_then(|ps| ps.into_iter().next())
            .map(|p| p.text)
            .unwrap_or_default();

        let (tokens_input, tokens_output) = resp.usage_metadata
            .map(|u| (
                u.prompt_token_count.unwrap_or(0),
                u.candidates_token_count.unwrap_or(0),
            ))
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
        if model.contains("flash") {
            (0.075, 0.30)
        } else if model.contains("pro") {
            (1.25, 5.0)
        } else {
            (0.075, 0.30)
        }
    }
}
