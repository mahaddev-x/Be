use anyhow::{anyhow, Result};
use reqwest::Client;
use std::time::Duration;

/// Fetch a URL and return cleaned plain text content (HTML stripped)
pub async fn fetch(url: &str) -> Result<String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Be/1.0 (local-first AI task runner)")
        .build()
        .map_err(|e| anyhow!("Failed to build HTTP client: {}", e))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow!("fetch_url: Failed to fetch '{}': {}", url, e))?;

    let status = response.status();
    if !status.is_success() {
        return Err(anyhow!("fetch_url: HTTP {} for '{}'", status, url));
    }

    let html = response.text().await
        .map_err(|e| anyhow!("fetch_url: Failed to read response body: {}", e))?;

    let text = html_to_text(&html);

    // Truncate to avoid overwhelming the LLM context
    let truncated = if text.len() > 8000 {
        format!("{}\n\n[Content truncated at 8000 chars]", &text[..8000])
    } else {
        text
    };

    Ok(truncated)
}

/// Convert HTML to plain text by stripping tags
fn html_to_text(html: &str) -> String {
    use scraper::{Html, Selector};

    let document = Html::parse_document(html);

    // Remove script and style elements
    let mut text_parts = Vec::new();

    // Walk the document and collect text nodes
    let body_selector = Selector::parse("body").unwrap_or_else(|_| Selector::parse("*").unwrap());

    let elements = document.select(&body_selector);
    for element in elements {
        for text_node in element.text() {
            let trimmed = text_node.trim();
            if !trimmed.is_empty() {
                text_parts.push(trimmed.to_string());
            }
        }
        break; // Just process the body element
    }

    if text_parts.is_empty() {
        // Fallback: simple regex-like strip
        strip_html_tags(html)
    } else {
        text_parts.join("\n")
    }
}

/// Simple HTML tag stripper fallback
fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_script = false;

    let html_lower = html.to_lowercase();
    let mut i = 0;
    let bytes = html.as_bytes();

    while i < bytes.len() {
        // Check for script/style start
        if !in_tag && i + 7 <= html.len() {
            let chunk = &html_lower[i..std::cmp::min(i + 8, html_lower.len())];
            if chunk.starts_with("<script") || chunk.starts_with("<style") {
                in_script = true;
            }
        }

        // Check for script/style end
        if in_script && i + 9 <= html.len() {
            let chunk = &html_lower[i..std::cmp::min(i + 9, html_lower.len())];
            if chunk.starts_with("</script") || chunk.starts_with("</style") {
                in_script = false;
                // Skip until end of tag
                while i < bytes.len() && bytes[i] != b'>' {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1;
                }
                continue;
            }
        }

        if in_script {
            i += 1;
            continue;
        }

        match bytes[i] {
            b'<' => {
                in_tag = true;
                i += 1;
            }
            b'>' => {
                in_tag = false;
                result.push(' ');
                i += 1;
            }
            _ if !in_tag => {
                result.push(bytes[i] as char);
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    // Clean up whitespace
    result
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
