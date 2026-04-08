use anyhow::{anyhow, Result};
use clap::Args;
use colored::Colorize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

use be_core::bee::{Bee, BeeInput};
use be_core::dispatcher::{dispatch, BeeEvent, DispatchConfig};
use be_core::store::JobStore;

#[derive(Args)]
pub struct RunArgs {
    /// Bee name to run
    pub bee: String,

    /// Input file: CSV, JSON array, or JSONL
    #[arg(long)]
    pub input: Option<String>,

    /// Inline JSON array input, e.g. '[{"url":"https://example.com"}]'
    #[arg(long = "input-json")]
    pub input_json: Option<String>,

    /// Read JSON array from stdin
    #[arg(long)]
    pub from_stdin: bool,

    /// Number of concurrent bees (default: 50, max: 1000)
    #[arg(long, default_value = "50")]
    pub parallel: usize,

    /// Override bee's model for this run
    #[arg(long)]
    pub model: Option<String>,

    /// Write results to file
    #[arg(long)]
    pub output: Option<String>,

    /// Output format: table (default) | json | csv | jsonl
    #[arg(long, default_value = "table")]
    pub format: String,

    /// Show live TUI grid during execution
    #[arg(long)]
    pub watch: bool,

    /// Plain text output (no colors)
    #[arg(long)]
    pub no_color: bool,
}

pub async fn run(mut args: RunArgs) -> Result<()> {
    let mut bee = Bee::find(&args.bee)?;

    // Override model if specified
    if let Some(model) = args.model.take() {
        bee.model = model;
    }

    // Load inputs
    let inputs = load_inputs(&args)?;
    if inputs.is_empty() {
        return Err(anyhow!("No inputs provided. Use --input, --input-json, or --from-stdin"));
    }

    println!(
        "{} Running {} {} on {} input(s) with {} parallel bees",
        "●".cyan(),
        bee.name.bold(),
        format!("({})", bee.model).dimmed(),
        inputs.len(),
        args.parallel
    );

    let store = Arc::new(JobStore::new()?);
    let (tx, mut rx) = mpsc::channel::<BeeEvent>(10_000);

    let config = DispatchConfig {
        parallel: args.parallel,
        progress_tx: tx,
    };

    let total = inputs.len();
    let job_id = dispatch(bee.clone(), inputs, config, Arc::clone(&store)).await?;

    println!("Job ID: {}", job_id.dimmed());

    // Progress tracking
    let mut completed = 0usize;
    let mut failed = 0usize;

    if args.watch {
        // TODO: TUI grid — for now fall through to simple progress
        println!("(--watch TUI coming soon, showing simple progress)");
    }

    // Consume events
    loop {
        match rx.recv().await {
            Some(BeeEvent::Completed { .. }) => {
                completed += 1;
                if !args.watch {
                    print_progress(completed, failed, total);
                }
            }
            Some(BeeEvent::Failed { error, index, .. }) => {
                failed += 1;
                if !args.watch {
                    print_progress(completed, failed, total);
                    eprintln!("  {} bee #{}: {}", "✗".red(), index, error);
                }
            }
            Some(BeeEvent::JobDone { .. }) => break,
            None => break,
            _ => {}
        }
    }

    println!(); // newline after progress

    // Load and display final results
    let results = store.load_results(&job_id)?;
    let manifest = store.load_manifest(&job_id)?;

    println!(
        "\n{} {} succeeded, {} failed | cost ${:.4} | avg {:.1}s/bee",
        "Done.".bold(),
        completed.to_string().green(),
        if failed > 0 { failed.to_string().red() } else { "0".normal() },
        manifest.total_cost_usd,
        manifest.avg_duration_ms as f64 / 1000.0,
    );
    println!("Results: be results {}", job_id);

    // Write output file if requested
    if let Some(output_path) = &args.output {
        let content = format_results(&results, &args.format)?;
        std::fs::write(output_path, content)?;
        println!("Output written to: {}", output_path);
    }

    Ok(())
}

fn load_inputs(args: &RunArgs) -> Result<Vec<BeeInput>> {
    if args.from_stdin {
        let mut stdin_content = String::new();
        std::io::stdin().lines().try_for_each(|l| -> Result<()> {
            stdin_content.push_str(&l?);
            stdin_content.push('\n');
            Ok(())
        })?;
        return parse_json_array(&stdin_content);
    }

    if let Some(inline) = &args.input_json {
        return parse_json_array(inline);
    }

    if let Some(file_path) = &args.input {
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| anyhow!("Cannot read input file '{}': {}", file_path, e))?;

        let ext = std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        return match ext {
            "json" => parse_json_array(&content),
            "jsonl" => parse_jsonl(&content),
            "csv" => parse_csv(&content),
            _ => {
                // Try JSON first, then JSONL, then CSV
                parse_json_array(&content)
                    .or_else(|_| parse_jsonl(&content))
                    .or_else(|_| parse_csv(&content))
            }
        };
    }

    Ok(Vec::new())
}

fn parse_json_array(s: &str) -> Result<Vec<BeeInput>> {
    let value: serde_json::Value = serde_json::from_str(s.trim())
        .map_err(|e| anyhow!("Invalid JSON: {}", e))?;

    let arr = value
        .as_array()
        .ok_or_else(|| anyhow!("Expected a JSON array of objects"))?;

    let inputs = arr
        .iter()
        .map(|item| {
            let obj = item.as_object()
                .ok_or_else(|| anyhow!("Each input must be a JSON object"))?;
            let mut map = HashMap::new();
            for (k, v) in obj {
                map.insert(k.clone(), match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                });
            }
            Ok(map)
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(inputs)
}

fn parse_jsonl(s: &str) -> Result<Vec<BeeInput>> {
    let mut inputs = Vec::new();
    for line in s.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(line)
            .map_err(|e| anyhow!("Invalid JSONL line: {}", e))?;
        let obj = value
            .as_object()
            .ok_or_else(|| anyhow!("Each JSONL line must be a JSON object"))?;
        let mut map = HashMap::new();
        for (k, v) in obj {
            map.insert(k.clone(), match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            });
        }
        inputs.push(map);
    }
    Ok(inputs)
}

fn parse_csv(s: &str) -> Result<Vec<BeeInput>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(s.as_bytes());

    let headers = reader.headers()?.clone();
    let mut inputs = Vec::new();

    for result in reader.records() {
        let record = result?;
        let mut map = HashMap::new();
        for (i, field) in record.iter().enumerate() {
            if let Some(header) = headers.get(i) {
                map.insert(header.to_string(), field.to_string());
            }
        }
        inputs.push(map);
    }

    Ok(inputs)
}

fn print_progress(completed: usize, failed: usize, total: usize) {
    let done = completed + failed;
    let pct = if total > 0 { done * 100 / total } else { 100 };
    let bar_width = 30;
    let filled = bar_width * done / total.max(1);
    let bar: String = format!(
        "{}{}",
        "█".repeat(filled),
        "░".repeat(bar_width - filled)
    );
    print!("\r  [{bar}] {done}/{total} ({pct}%)  ✓ {completed}  ✗ {failed}  ");
    use std::io::Write;
    let _ = std::io::stdout().flush();
}

fn format_results(
    results: &[be_core::store::BeeResult],
    format: &str,
) -> Result<String> {
    match format {
        "json" => Ok(serde_json::to_string_pretty(results)?),
        "jsonl" => {
            let lines: Vec<String> = results
                .iter()
                .map(|r| serde_json::to_string(r))
                .collect::<std::result::Result<_, _>>()?;
            Ok(lines.join("\n"))
        }
        "csv" => {
            let mut wtr = csv::Writer::from_writer(vec![]);
            for r in results {
                let status = r.status.to_string();
                let output = r.output.as_ref()
                    .map(|o| serde_json::to_string(o).unwrap_or_default())
                    .unwrap_or_default();
                let error = r.error.as_deref().unwrap_or("");
                wtr.write_record(&[
                    r.index.to_string(),
                    status,
                    output,
                    error.to_string(),
                    r.cost_usd.to_string(),
                    r.duration_ms.to_string(),
                ])?;
            }
            Ok(String::from_utf8(wtr.into_inner()?)?)
        }
        _ => Ok(String::new()), // table format handled by results command
    }
}
