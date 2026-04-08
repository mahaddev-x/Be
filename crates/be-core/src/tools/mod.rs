pub mod fetch_url;
pub mod read_file;

use anyhow::{anyhow, Result};
use crate::bee::{Bee, BeeInput};

/// Run all pre-tools defined in the bee to enrich the input before LLM call
pub async fn run_pre_tools(bee: &Bee, input: &BeeInput) -> Result<BeeInput> {
    let mut enriched = input.clone();

    for tool_name in &bee.tools {
        match tool_name.as_str() {
            "fetch_url" => {
                let url = enriched.get("url")
                    .cloned()
                    .ok_or_else(|| anyhow!(
                        "Tool 'fetch_url' requires 'url' in input_vars, but it's missing"
                    ))?;
                let content = fetch_url::fetch(&url).await?;
                enriched.insert("content".to_string(), content);
            }
            "read_file" => {
                let path = enriched.get("file_path")
                    .cloned()
                    .ok_or_else(|| anyhow!(
                        "Tool 'read_file' requires 'file_path' in input_vars, but it's missing"
                    ))?;
                let content = read_file::read(&path)?;
                enriched.insert("file_content".to_string(), content);
            }
            other => return Err(anyhow!(
                "Unknown tool: '{}'. Supported tools: fetch_url, read_file", other
            )),
        }
    }

    Ok(enriched)
}
