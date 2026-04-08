use anyhow::Result;
use chrono::Utc;
use std::time::{Duration, Instant};

use crate::bee::{Bee, BeeInput};
use crate::llm::{self, LlmRequest};
use crate::schema;
use crate::store::{BeeResult, ResultStatus};
use crate::tools;

pub async fn run_bee(
    bee: &Bee,
    input: BeeInput,
    index: usize,
) -> Result<BeeResult> {
    let start = Instant::now();
    let mut attempt = 0u32;

    loop {
        attempt += 1;
        match try_run_bee(bee, &input, index, &start).await {
            Ok(mut result) => {
                result.retries_attempted = attempt - 1;
                return Ok(result);
            }
            Err(_) if attempt < bee.retry.max_attempts => {
                tokio::time::sleep(Duration::from_secs(bee.retry.backoff_seconds)).await;
            }
            Err(e) => {
                return Ok(BeeResult {
                    index,
                    status: ResultStatus::Failed,
                    input,
                    output: None,
                    error: Some(e.to_string()),
                    retries_attempted: attempt - 1,
                    tokens_input: 0,
                    tokens_output: 0,
                    cost_usd: 0.0,
                    duration_ms: start.elapsed().as_millis() as u64,
                    model_used: bee.model.clone(),
                    completed_at: Utc::now(),
                });
            }
        }
    }
}

async fn try_run_bee(
    bee: &Bee,
    input: &BeeInput,
    index: usize,
    start: &Instant,
) -> Result<BeeResult> {
    // Step 1: Run pre-tools to enrich input
    let enriched = tools::run_pre_tools(bee, input).await?;

    // Step 2: Interpolate prompt template
    let user_prompt = bee.interpolate_prompt(&enriched);

    // Step 3: Call LLM
    let llm_response = llm::complete(LlmRequest {
        model: &bee.model,
        system: &bee.system_prompt,
        user: &user_prompt,
        temperature: bee.temperature,
        max_tokens: bee.max_tokens,
        timeout: Duration::from_secs(bee.timeout_seconds),
    })
    .await?;

    // Step 4: Validate output JSON
    let validated_output = schema::validate_and_parse(
        &llm_response.content,
        &bee.output_schema,
    )?;

    Ok(BeeResult {
        index,
        status: ResultStatus::Success,
        input: input.clone(),
        output: Some(validated_output),
        error: None,
        retries_attempted: 0,
        tokens_input: llm_response.tokens_input,
        tokens_output: llm_response.tokens_output,
        cost_usd: llm_response.cost_usd,
        duration_ms: start.elapsed().as_millis() as u64,
        model_used: bee.model.clone(),
        completed_at: Utc::now(),
    })
}
