use anyhow::Result;
use colored::Colorize;
use dialoguer::{Confirm, Input, MultiSelect};

use be_core::config::{self, Config};

pub async fn run() -> Result<()> {
    println!("{}", "Welcome to Be!\n".bold().cyan());
    println!("Be runs hundreds of independent AI tasks in parallel.");
    println!("Let's get you set up.\n");

    let providers = &[
        "Groq (fast, cheap, Llama/Mixtral/Gemma models)",
        "Ollama (local, free, GPU-accelerated)",
        "Anthropic (Claude models, quality-focused)",
        "OpenAI",
        "OpenRouter (200+ models via one key)",
        "DeepSeek (very cheap)",
        "Together AI",
        "Google (Gemini models)",
    ];

    let selected = MultiSelect::new()
        .with_prompt("Which providers do you want to use?")
        .items(providers)
        .defaults(&[true, true, false, false, false, false, false, false])
        .interact()?;

    let mut cfg = config::load().unwrap_or_default();

    for idx in &selected {
        match *idx {
            0 => {
                // Groq
                println!("\nEnter your Groq API key (free at console.groq.com):");
                let key: String = Input::new()
                    .with_prompt("> ")
                    .allow_empty(true)
                    .interact()?;
                if !key.is_empty() {
                    cfg.providers.groq_api_key = key;
                }
            }
            1 => {
                // Ollama
                let url = cfg.providers.ollama_url.clone();
                print!("\nOllama: checking {} ... ", url);
                match reqwest::get(&format!("{}/api/tags", url)).await {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(body) = resp.text().await {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                                let empty = vec![];
                                let models: Vec<&str> = json["models"]
                                    .as_array()
                                    .unwrap_or(&empty)
                                    .iter()
                                    .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                                    .collect();
                                if models.is_empty() {
                                    println!("{} (no models pulled yet)", "✓ Found!".green());
                                    println!(
                                        "  Pull a model with: ollama pull qwen2.5:3b"
                                    );
                                } else {
                                    println!(
                                        "{} Available models: {}",
                                        "✓ Found!".green(),
                                        models.join(", ")
                                    );
                                }
                            }
                        }
                    }
                    _ => {
                        println!("{}", "✗ Not reachable".red());
                        println!("  Install Ollama from https://ollama.com");
                        let custom_url: String = Input::new()
                            .with_prompt("Custom Ollama URL (or Enter to skip)")
                            .with_initial_text(&url)
                            .allow_empty(true)
                            .interact()?;
                        if !custom_url.is_empty() {
                            cfg.providers.ollama_url = custom_url;
                        }
                    }
                }
            }
            2 => {
                println!("\nEnter your Anthropic API key:");
                let key: String = Input::new().with_prompt("> ").allow_empty(true).interact()?;
                if !key.is_empty() { cfg.providers.anthropic_api_key = key; }
            }
            3 => {
                println!("\nEnter your OpenAI API key:");
                let key: String = Input::new().with_prompt("> ").allow_empty(true).interact()?;
                if !key.is_empty() { cfg.providers.openai_api_key = key; }
            }
            4 => {
                println!("\nEnter your OpenRouter API key:");
                let key: String = Input::new().with_prompt("> ").allow_empty(true).interact()?;
                if !key.is_empty() { cfg.providers.openrouter_api_key = key; }
            }
            5 => {
                println!("\nEnter your DeepSeek API key:");
                let key: String = Input::new().with_prompt("> ").allow_empty(true).interact()?;
                if !key.is_empty() { cfg.providers.deepseek_api_key = key; }
            }
            6 => {
                println!("\nEnter your Together AI API key:");
                let key: String = Input::new().with_prompt("> ").allow_empty(true).interact()?;
                if !key.is_empty() { cfg.providers.together_api_key = key; }
            }
            7 => {
                println!("\nEnter your Google AI API key:");
                let key: String = Input::new().with_prompt("> ").allow_empty(true).interact()?;
                if !key.is_empty() { cfg.providers.google_api_key = key; }
            }
            _ => {}
        }
    }

    config::save(&cfg)?;
    println!("\n{}", format!("Config saved to {:?}", config::config_path()).green());

    println!("\n{}", "Run your first bee:".bold());
    println!("  be bee list");
    println!("  be bee test url-scraper --input '{{\"url\":\"https://example.com\"}}'");
    println!("  be run url-scraper --input urls.csv --parallel 50");

    Ok(())
}
