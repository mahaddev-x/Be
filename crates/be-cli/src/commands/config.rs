use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;

use be_core::config;

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Set a config value (e.g. be config set providers.groq_api_key gsk_...)
    Set {
        key: String,
        value: String,
    },
    /// Get a config value
    Get {
        key: String,
    },
    /// Show full config (API keys masked)
    Show,
    /// Test all provider connections
    Test,
}

pub async fn run(cmd: ConfigCommands) -> Result<()> {
    match cmd {
        ConfigCommands::Set { key, value } => set_config(&key, &value),
        ConfigCommands::Get { key } => get_config(&key),
        ConfigCommands::Show => show_config(),
        ConfigCommands::Test => test_config().await,
    }
}

fn set_config(key: &str, value: &str) -> Result<()> {
    let mut cfg = config::load()?;
    cfg.set_key(key, value)?;
    config::save(&cfg)?;
    println!("{} {} = {}", "✓".green(), key, if key.contains("api_key") { "***" } else { value });
    Ok(())
}

fn get_config(key: &str) -> Result<()> {
    let cfg = config::load()?;
    let value = cfg.get_key(key)?;
    let display = if key.contains("api_key") && !value.is_empty() {
        mask_key(&value)
    } else {
        value
    };
    println!("{} = {}", key, display);
    Ok(())
}

fn show_config() -> Result<()> {
    let cfg = config::load()?;
    let path = config::config_path();
    println!("{}", format!("Config: {:?}", path).bold());
    println!();
    println!("[providers]");
    println!("  groq_api_key       = {}", mask_key_if_set(&cfg.providers.groq_api_key));
    println!("  anthropic_api_key  = {}", mask_key_if_set(&cfg.providers.anthropic_api_key));
    println!("  openai_api_key     = {}", mask_key_if_set(&cfg.providers.openai_api_key));
    println!("  openrouter_api_key = {}", mask_key_if_set(&cfg.providers.openrouter_api_key));
    println!("  together_api_key   = {}", mask_key_if_set(&cfg.providers.together_api_key));
    println!("  deepseek_api_key   = {}", mask_key_if_set(&cfg.providers.deepseek_api_key));
    println!("  google_api_key     = {}", mask_key_if_set(&cfg.providers.google_api_key));
    println!("  ollama_url         = {}", cfg.providers.ollama_url);
    println!();
    println!("[defaults]");
    println!("  model              = {}", cfg.defaults.model);
    println!("  parallel           = {}", cfg.defaults.parallel);
    println!("  timeout_seconds    = {}", cfg.defaults.timeout_seconds);
    println!("  max_retries        = {}", cfg.defaults.max_retries);
    Ok(())
}

async fn test_config() -> Result<()> {
    let cfg = config::load()?;

    println!("{}", "Testing provider connections...\n".bold());

    // Test Ollama
    let ollama_url = cfg.ollama_url();
    print!("  Ollama ({}) ... ", ollama_url);
    match reqwest::get(&format!("{}/api/tags", ollama_url)).await {
        Ok(resp) if resp.status().is_success() => {
            println!("{}", "✓ Connected".green());
        }
        _ => println!("{}", "✗ Not reachable".red()),
    }

    // Check which API keys are set
    let providers = [
        ("Groq", !cfg.providers.groq_api_key.is_empty()),
        ("Anthropic", !cfg.providers.anthropic_api_key.is_empty()),
        ("OpenAI", !cfg.providers.openai_api_key.is_empty()),
        ("OpenRouter", !cfg.providers.openrouter_api_key.is_empty()),
        ("Together", !cfg.providers.together_api_key.is_empty()),
        ("DeepSeek", !cfg.providers.deepseek_api_key.is_empty()),
        ("Google", !cfg.providers.google_api_key.is_empty()),
    ];

    for (name, has_key) in &providers {
        if *has_key {
            println!("  {} {} API key set", "✓".green(), name);
        } else {
            println!("  {} {} not configured", "○".dimmed(), name);
        }
    }

    Ok(())
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "*".repeat(key.len())
    } else {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    }
}

fn mask_key_if_set(key: &str) -> String {
    if key.is_empty() {
        "(not set)".dimmed().to_string()
    } else {
        mask_key(key)
    }
}
