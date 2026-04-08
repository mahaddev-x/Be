use anyhow::Result;
use clap::Args;
use colored::Colorize;
use comfy_table::{Table, presets::UTF8_FULL};

use be_core::store::{JobStore, ResultStatus};

#[derive(Args)]
pub struct ResultsArgs {
    /// Job ID
    pub job_id: String,

    /// Output format: table (default) | json | csv | jsonl
    #[arg(long, default_value = "table")]
    pub format: String,

    /// Maximum number of results to show
    #[arg(long)]
    pub limit: Option<usize>,

    /// Offset (skip N results)
    #[arg(long, default_value = "0")]
    pub offset: usize,

    /// Show only failed results
    #[arg(long)]
    pub failed_only: bool,
}

pub async fn run(args: ResultsArgs) -> Result<()> {
    let store = JobStore::new()?;
    let manifest = store.load_manifest(&args.job_id)?;
    let mut results = store.load_results(&args.job_id)?;

    if args.failed_only {
        results.retain(|r| r.status == ResultStatus::Failed);
    }

    let total_before_limit = results.len();
    let results: Vec<_> = results
        .into_iter()
        .skip(args.offset)
        .take(args.limit.unwrap_or(usize::MAX))
        .collect();

    match args.format.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&results)?);
        }
        "jsonl" => {
            for r in &results {
                println!("{}", serde_json::to_string(r)?);
            }
        }
        "csv" => {
            let mut wtr = csv::Writer::from_writer(std::io::stdout());
            wtr.write_record(["index", "status", "input", "output", "error", "cost_usd", "duration_ms", "model_used"])?;
            for r in &results {
                wtr.write_record([
                    &r.index.to_string(),
                    &r.status.to_string(),
                    &serde_json::to_string(&r.input).unwrap_or_default(),
                    &r.output.as_ref().map(|o| serde_json::to_string(o).unwrap_or_default()).unwrap_or_default(),
                    r.error.as_deref().unwrap_or(""),
                    &r.cost_usd.to_string(),
                    &r.duration_ms.to_string(),
                    &r.model_used,
                ])?;
            }
            wtr.flush()?;
        }
        _ => {
            // Table format
            println!(
                "{} · {} · {}",
                format!("Job {}", &args.job_id[..8]).bold(),
                manifest.bee_name.cyan(),
                manifest.model.dimmed()
            );
            println!("Status: {}  Total: {}  Completed: {}  Failed: {}",
                manifest.status.to_string().bold(),
                manifest.total,
                manifest.completed.to_string().green(),
                if manifest.failed > 0 { manifest.failed.to_string().red() } else { "0".normal() },
            );
            println!();

            let mut table = Table::new();
            table.load_preset(UTF8_FULL);
            table.set_header(["#", "STATUS", "INPUT (summary)", "OUTPUT (summary)", "COST", "TIME"]);

            for r in &results {
                let input_summary = summarize_map(&r.input, 40);
                let output_summary = r.output.as_ref()
                    .map(|o| summarize_json(o, 40))
                    .unwrap_or_else(|| r.error.as_deref().unwrap_or("—").to_string());
                let status_icon = match r.status {
                    ResultStatus::Success => "✓".green().to_string(),
                    ResultStatus::Failed => "✗".red().to_string(),
                };
                let cost = if r.cost_usd > 0.0 {
                    format!("${:.4}", r.cost_usd)
                } else {
                    "—".to_string()
                };
                let time = format!("{:.1}s", r.duration_ms as f64 / 1000.0);

                table.add_row([
                    &r.index.to_string(),
                    &status_icon,
                    &input_summary,
                    &output_summary,
                    &cost,
                    &time,
                ]);
            }

            println!("{}", table);

            if total_before_limit > results.len() {
                println!(
                    "Showing {}/{} results. Use --offset and --limit to paginate.",
                    results.len(),
                    total_before_limit
                );
            }

            println!(
                "\n{} total · {} succeeded · {} failed · total cost ${:.4} · avg {:.1}s/bee",
                manifest.total,
                manifest.completed.to_string().green(),
                manifest.failed,
                manifest.total_cost_usd,
                manifest.avg_duration_ms as f64 / 1000.0,
            );
        }
    }

    Ok(())
}

fn summarize_map(map: &std::collections::HashMap<String, String>, max_len: usize) -> String {
    let parts: Vec<String> = map.iter()
        .take(2)
        .map(|(k, v)| {
            let v_short = if v.len() > 20 { format!("{}…", &v[..17]) } else { v.clone() };
            format!("{}={}", k, v_short)
        })
        .collect();
    let s = parts.join(", ");
    if s.len() > max_len { format!("{}…", &s[..max_len - 1]) } else { s }
}

fn summarize_json(v: &serde_json::Value, max_len: usize) -> String {
    let s = match v {
        serde_json::Value::Object(obj) => {
            obj.iter().take(2)
                .map(|(k, val)| {
                    let vs = match val {
                        serde_json::Value::String(s) => {
                            if s.len() > 15 { format!("{}…", &s[..12]) } else { s.clone() }
                        }
                        other => other.to_string(),
                    };
                    format!("{}={}", k, vs)
                })
                .collect::<Vec<_>>()
                .join(", ")
        }
        other => other.to_string(),
    };
    if s.len() > max_len { format!("{}…", &s[..max_len - 1]) } else { s }
}
