use anyhow::{anyhow, Result};

/// Read a local file and return its content as a string
pub fn read(path: &str) -> Result<String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow!("read_file: Failed to read '{}': {}", path, e))?;

    // Truncate very large files
    let truncated = if content.len() > 16_000 {
        format!("{}\n\n[File truncated at 16000 chars]", &content[..16_000])
    } else {
        content
    };

    Ok(truncated)
}
