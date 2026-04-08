use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;

use be_core::bee::Bee;

#[derive(Subcommand)]
pub enum BeeCommands {
    /// Create a new bee interactively
    New,
    /// Load and validate a bee from a YAML file
    Create {
        /// Path to the bee YAML file
        file: String,
    },
    /// List all available bees
    List,
    /// Show a bee's definition
    Show {
        /// Bee name
        name: String,
    },
    /// Open a bee YAML in $EDITOR
    Edit {
        /// Bee name
        name: String,
    },
    /// Run one bee on one input to test it
    Test {
        /// Bee name
        name: String,
        /// Input as JSON object, e.g. '{"url":"https://example.com"}'
        #[arg(long)]
        input: Option<String>,
    },
}

pub async fn run(cmd: BeeCommands) -> Result<()> {
    match cmd {
        BeeCommands::New => new_bee().await,
        BeeCommands::Create { file } => create_bee(&file),
        BeeCommands::List => list_bees(),
        BeeCommands::Show { name } => show_bee(&name),
        BeeCommands::Edit { name } => edit_bee(&name),
        BeeCommands::Test { name, input } => test_bee(&name, input.as_deref()).await,
    }
}

async fn new_bee() -> Result<()> {
    use dialoguer::{Confirm, Input, Select};

    println!("{}", "Creating a new bee.\n".bold());

    let name: String = Input::new()
        .with_prompt("Bee name (snake_case)")
        .validate_with(|s: &String| {
            if s.is_empty() {
                Err("Name cannot be empty")
            } else {
                Ok(())
            }
        })
        .interact()?;

    let description: String = Input::new()
        .with_prompt("What does this bee do? (one sentence)")
        .interact()?;

    let models = &[
        "groq/llama-3.1-8b-instant  (fastest, cheapest — recommended)",
        "groq/llama-3.3-70b-versatile",
        "groq/gemma2-9b-it",
        "ollama/qwen2.5:3b  (local, free)",
        "anthropic/claude-haiku-4-5",
        "google/gemini-flash-2.0",
        "custom model string",
    ];
    let model_values = &[
        "groq/llama-3.1-8b-instant",
        "groq/llama-3.3-70b-versatile",
        "groq/gemma2-9b-it",
        "ollama/qwen2.5:3b",
        "anthropic/claude-haiku-4-5",
        "google/gemini-flash-2.0",
        "",
    ];

    let model_idx = Select::new()
        .with_prompt("Which model?")
        .items(models)
        .default(0)
        .interact()?;

    let model = if model_idx == models.len() - 1 {
        Input::<String>::new()
            .with_prompt("Custom model string (provider/model)")
            .interact()?
    } else {
        model_values[model_idx].to_string()
    };

    // Collect input variables
    println!("\n{}", "Input variables:".bold());
    let mut input_vars = Vec::new();
    let types = &["string", "integer", "boolean"];

    loop {
        let var_name: String = Input::new()
            .with_prompt("  Add variable name (Enter to skip)")
            .allow_empty(true)
            .interact()?;

        if var_name.is_empty() {
            break;
        }

        let type_idx = Select::new()
            .with_prompt("  Type")
            .items(types)
            .default(0)
            .interact()?;

        let required = Confirm::new()
            .with_prompt("  Required?")
            .default(true)
            .interact()?;

        input_vars.push(serde_json::json!({
            "name": var_name,
            "type": types[type_idx],
            "required": required,
        }));
    }

    // Collect output fields
    println!("\n{}", "Output fields:".bold());
    let output_types = &["string", "integer", "boolean", "array", "number"];
    let mut output_schema = serde_json::Map::new();

    loop {
        let field_name: String = Input::new()
            .with_prompt("  Add field name (Enter to skip)")
            .allow_empty(true)
            .interact()?;

        if field_name.is_empty() {
            break;
        }

        let type_idx = Select::new()
            .with_prompt("  Type")
            .items(output_types)
            .default(0)
            .interact()?;

        output_schema.insert(field_name, serde_json::json!(output_types[type_idx]));
    }

    // Tools
    println!();
    let tool_options = &[
        "Yes, add fetch_url tool (fetches {{url}} content automatically)",
        "Yes, add read_file tool (reads {{file_path}} content automatically)",
        "No tools needed",
    ];
    let tool_idx = Select::new()
        .with_prompt("Does this bee need to fetch data before the LLM call?")
        .items(tool_options)
        .default(2)
        .interact()?;

    let tools: Vec<&str> = match tool_idx {
        0 => vec!["fetch_url"],
        1 => vec!["read_file"],
        _ => vec![],
    };

    // Build user prompt template
    let input_var_names: Vec<String> = input_vars
        .iter()
        .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
        .collect();

    let mut prompt_vars = input_var_names.join(", ");
    if tools.contains(&"fetch_url") {
        prompt_vars.push_str(", content");
    }
    if tools.contains(&"read_file") {
        prompt_vars.push_str(", file_content");
    }

    let user_prompt = format!(
        "Process the following:\n\n{}\n\nReturn JSON only.",
        input_var_names
            .iter()
            .map(|n| format!("{}: {{{{{}}}}}", n, n))
            .collect::<Vec<_>>()
            .join("\n")
    );

    // Build YAML content
    let input_vars_yaml = input_vars
        .iter()
        .filter_map(|v| {
            let name = v.get("name")?.as_str()?;
            let type_ = v.get("type")?.as_str()?;
            let required = v.get("required")?.as_bool()?;
            Some(format!(
                "  - name: {}\n    type: {}\n    required: {}",
                name, type_, required
            ))
        })
        .collect::<Vec<_>>()
        .join("\n");

    let output_schema_yaml = output_schema
        .iter()
        .map(|(k, v)| format!("  {}: {}", k, v.as_str().unwrap_or("string")))
        .collect::<Vec<_>>()
        .join("\n");

    let tools_yaml = if tools.is_empty() {
        "tools: []".to_string()
    } else {
        format!("tools:\n{}", tools.iter().map(|t| format!("  - {}", t)).collect::<Vec<_>>().join("\n"))
    };

    let yaml_content = format!(
        r#"name: {name}
version: "1.0"
description: "{description}"

model: {model}

temperature: 0.1
max_tokens: 512
timeout_seconds: 30
max_parallel: 200

system_prompt: |
  You are a precise AI task executor. Given the input, complete your task
  and return ONLY a valid JSON object matching the output schema exactly.
  No explanation. No markdown. No text outside the JSON object.

user_prompt_template: |
  {user_prompt}

input_vars:
{input_vars_yaml}

output_schema:
{output_schema_yaml}

{tools_yaml}

retry:
  max_attempts: 3
  backoff_seconds: 2
"#,
        name = name,
        description = description,
        model = model,
        user_prompt = user_prompt,
        input_vars_yaml = input_vars_yaml,
        output_schema_yaml = output_schema_yaml,
        tools_yaml = tools_yaml,
    );

    // Validate
    let bee = Bee::from_yaml_str(&yaml_content)?;

    // Write file
    std::fs::create_dir_all("bees")?;
    let file_path = format!("bees/{}.yaml", name);
    std::fs::write(&file_path, &yaml_content)?;
    println!("\n{} {}", "Writing to".green(), file_path.bold());

    // Offer to test
    let test_now = Confirm::new()
        .with_prompt("Test it now?")
        .default(true)
        .interact()?;

    if test_now {
        let input_json: String = Input::new()
            .with_prompt("Test input (JSON object)")
            .with_initial_text("{}")
            .interact()?;

        run_test(&bee, &input_json).await?;
    } else {
        println!("\n{}", "Run a swarm:".bold());
        println!("  be run {} --input data.csv --parallel 100", name);
    }

    Ok(())
}

fn create_bee(file: &str) -> Result<()> {
    let bee = Bee::from_yaml_file(std::path::Path::new(file))?;
    println!("{} '{}' loaded and validated successfully.", "✓".green().bold(), bee.name);
    println!("  Model: {}", bee.model);
    println!("  Description: {}", bee.description);
    println!("  Tools: {}", if bee.tools.is_empty() { "none".to_string() } else { bee.tools.join(", ") });
    Ok(())
}

fn list_bees() -> Result<()> {
    let names = Bee::list_all();
    if names.is_empty() {
        println!("No bees found. Run `be bee new` to create one.");
        return Ok(());
    }

    println!("{:<30} {:<50} {}", "NAME".bold(), "DESCRIPTION".bold(), "MODEL".bold());
    println!("{}", "─".repeat(90));

    for name in &names {
        match Bee::find(name) {
            Ok(bee) => {
                println!("{:<30} {:<50} {}", name, truncate(&bee.description, 48), bee.model);
            }
            Err(_) => {
                println!("{:<30} {}", name, "(failed to load)".red());
            }
        }
    }

    println!("\n{} bee(s) available", names.len());
    Ok(())
}

fn show_bee(name: &str) -> Result<()> {
    let bee = Bee::find(name)?;
    println!("{}", format!("Bee: {}", bee.name).bold());
    println!("Version: {}", bee.version);
    println!("Description: {}", bee.description);
    println!("Model: {}", bee.model);
    println!("Temperature: {}", bee.temperature);
    println!("Max tokens: {}", bee.max_tokens);
    println!("Timeout: {}s", bee.timeout_seconds);
    println!("Max parallel: {}", bee.max_parallel);
    println!("Tools: {}", if bee.tools.is_empty() { "none".to_string() } else { bee.tools.join(", ") });
    println!("Retry: {} attempts, {}s backoff", bee.retry.max_attempts, bee.retry.backoff_seconds);
    println!("\n{}", "Input vars:".bold());
    for v in &bee.input_vars {
        println!("  - {} ({}) {}", v.name, v.type_, if v.required { "required" } else { "optional" });
    }
    println!("\n{}", "Output schema:".bold());
    println!("{}", serde_json::to_string_pretty(&bee.output_schema)?);
    println!("\n{}", "System prompt:".bold());
    println!("{}", bee.system_prompt);
    println!("\n{}", "User prompt template:".bold());
    println!("{}", bee.user_prompt_template);
    Ok(())
}

fn edit_bee(name: &str) -> Result<()> {
    // Find the bee file path
    let local = std::path::Path::new("bees").join(format!("{}.yaml", name));
    let user_path = dirs::home_dir()
        .map(|h| h.join(".be").join("bees").join(format!("{}.yaml", name)));

    let path = if local.exists() {
        local
    } else if let Some(up) = user_path.filter(|p| p.exists()) {
        up
    } else {
        return Err(anyhow::anyhow!(
            "Bee '{}' not found in ./bees/ or ~/.be/bees/. Built-in bees cannot be edited directly.",
            name
        ));
    };

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    std::process::Command::new(&editor)
        .arg(&path)
        .status()?;

    // Validate after edit
    match Bee::from_yaml_file(&path) {
        Ok(bee) => println!("{} Bee '{}' updated and valid.", "✓".green(), bee.name),
        Err(e) => eprintln!("{} Bee YAML is invalid after edit: {}", "✗".red(), e),
    }

    Ok(())
}

async fn test_bee(name: &str, input_str: Option<&str>) -> Result<()> {
    let bee = Bee::find(name)?;

    let input_json = match input_str {
        Some(s) => s.to_string(),
        None => {
            use dialoguer::Input;
            Input::<String>::new()
                .with_prompt("Test input (JSON object)")
                .with_initial_text("{}")
                .interact()?
        }
    };

    run_test(&bee, &input_json).await
}

async fn run_test(bee: &Bee, input_json: &str) -> Result<()> {
    use std::collections::HashMap;

    let raw: serde_json::Value = serde_json::from_str(input_json)?;
    let mut input: HashMap<String, String> = HashMap::new();
    if let Some(obj) = raw.as_object() {
        for (k, v) in obj {
            input.insert(k.clone(), match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            });
        }
    }

    println!("Running {} on test input...", bee.name);
    let start = std::time::Instant::now();

    match be_core::runner::run_bee(bee, input, 0).await {
        Ok(result) => {
            let elapsed = start.elapsed();
            if result.status == be_core::store::ResultStatus::Success {
                println!("\n{}", "Result:".bold());
                if let Some(output) = &result.output {
                    println!("{}", serde_json::to_string_pretty(output)?);
                }
                println!(
                    "\n  Tokens: {} in / {} out  |  Cost: ${:.4}  |  Time: {:.1}s  |  Status: {}",
                    result.tokens_input,
                    result.tokens_output,
                    result.cost_usd,
                    elapsed.as_secs_f64(),
                    "✓".green()
                );
            } else {
                println!("{} {}", "✗ Failed:".red(), result.error.unwrap_or_default());
            }
        }
        Err(e) => {
            eprintln!("{} {}", "✗ Error:".red(), e);
        }
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}
