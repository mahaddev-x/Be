use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::{BufRead, Write};
use std::sync::Arc;
use tokio::sync::mpsc;

use be_core::bee::Bee;
use be_core::dispatcher::{dispatch, BeeEvent, DispatchConfig};
use be_core::store::JobStore;

// MCP JSON-RPC types
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    id: Option<serde_json::Value>,
    method: String,
    params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

impl JsonRpcResponse {
    fn ok(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn err(id: Option<serde_json::Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError { code, message }),
        }
    }
}

/// List of MCP tools exposed by Be
fn tools_list() -> serde_json::Value {
    json!({
        "tools": [
            {
                "name": "be_run",
                "description": "Deploy N bees to process a list of inputs in parallel. Returns job_id.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "bee":      { "type": "string",  "description": "Bee name" },
                        "inputs":   { "type": "array",   "description": "Array of input objects" },
                        "parallel": { "type": "integer", "description": "Concurrent bees (1-1000)", "default": 50 },
                        "wait":     { "type": "boolean", "description": "Wait for completion", "default": true }
                    },
                    "required": ["bee", "inputs"]
                }
            },
            {
                "name": "be_status",
                "description": "Get status and progress of a job.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "job_id": { "type": "string" }
                    },
                    "required": ["job_id"]
                }
            },
            {
                "name": "be_results",
                "description": "Get results of a completed job.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "job_id":      { "type": "string" },
                        "limit":       { "type": "integer" },
                        "offset":      { "type": "integer" },
                        "failed_only": { "type": "boolean" }
                    },
                    "required": ["job_id"]
                }
            },
            {
                "name": "be_list_bees",
                "description": "List all available bees with their descriptions and input schemas."
            },
            {
                "name": "be_test",
                "description": "Test one bee with one input. Runs synchronously, returns result immediately.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "bee":   { "type": "string" },
                        "input": { "type": "object" }
                    },
                    "required": ["bee", "input"]
                }
            }
        ]
    })
}

pub async fn run() -> Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let store = Arc::new(JobStore::new()?);

    let mut lines = stdin.lock().lines();

    loop {
        let line = match lines.next() {
            Some(Ok(l)) => l,
            Some(Err(e)) => return Err(e.into()),
            None => break, // EOF
        };

        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse::err(None, -32700, format!("Parse error: {}", e));
                let json = serde_json::to_string(&resp)?;
                let mut out = stdout.lock();
                writeln!(out, "{}", json)?;
                out.flush()?;
                continue;
            }
        };

        let id = req.id.clone();
        let resp = handle_request(req, Arc::clone(&store)).await;

        let json = serde_json::to_string(&resp)?;
        let mut out = stdout.lock();
        writeln!(out, "{}", json)?;
        out.flush()?;
    }

    Ok(())
}

async fn handle_request(req: JsonRpcRequest, store: Arc<JobStore>) -> JsonRpcResponse {
    let id = req.id.clone();

    match req.method.as_str() {
        "initialize" => {
            JsonRpcResponse::ok(id, json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "be", "version": "1.0.0" }
            }))
        }
        "tools/list" => JsonRpcResponse::ok(id, tools_list()),
        "tools/call" => {
            let params = req.params.unwrap_or(json!({}));
            let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));

            match tool_call(tool_name, args, Arc::clone(&store)).await {
                Ok(result) => JsonRpcResponse::ok(id, json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&result).unwrap_or_default() }]
                })),
                Err(e) => JsonRpcResponse::err(id, -32000, e.to_string()),
            }
        }
        other => JsonRpcResponse::err(id, -32601, format!("Method not found: {}", other)),
    }
}

async fn tool_call(
    name: &str,
    args: serde_json::Value,
    store: Arc<JobStore>,
) -> Result<serde_json::Value> {
    match name {
        "be_list_bees" => {
            let bee_names = Bee::list_all();
            let bees: Vec<serde_json::Value> = bee_names
                .iter()
                .filter_map(|n| Bee::find(n).ok())
                .map(|b| json!({
                    "name": b.name,
                    "description": b.description,
                    "model": b.model,
                    "input_vars": b.input_vars.iter().map(|v| json!({
                        "name": v.name,
                        "type": v.type_,
                        "required": v.required,
                    })).collect::<Vec<_>>(),
                    "output_schema": b.output_schema,
                }))
                .collect();
            Ok(json!({ "bees": bees }))
        }

        "be_run" => {
            let bee_name = args["bee"].as_str()
                .ok_or_else(|| anyhow!("Missing 'bee' argument"))?;
            let inputs_raw = args["inputs"].as_array()
                .ok_or_else(|| anyhow!("Missing 'inputs' argument"))?;
            let parallel = args.get("parallel").and_then(|p| p.as_u64()).unwrap_or(50) as usize;
            let wait = args.get("wait").and_then(|w| w.as_bool()).unwrap_or(true);

            let bee = Bee::find(bee_name)?;
            let inputs: Vec<be_core::bee::BeeInput> = inputs_raw
                .iter()
                .map(|v| {
                    let obj = v.as_object()
                        .ok_or_else(|| anyhow!("Each input must be a JSON object"))?;
                    let mut map = std::collections::HashMap::new();
                    for (k, val) in obj {
                        map.insert(k.clone(), match val {
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        });
                    }
                    Ok(map)
                })
                .collect::<Result<Vec<_>>>()?;

            let (tx, mut rx) = mpsc::channel::<BeeEvent>(10_000);
            let config = DispatchConfig { parallel, progress_tx: tx };
            let job_id = dispatch(bee, inputs, config, Arc::clone(&store)).await?;

            if wait {
                // Wait for JobDone
                loop {
                    match rx.recv().await {
                        Some(BeeEvent::JobDone { completed, failed, .. }) => {
                            let manifest = store.load_manifest(&job_id)?;
                            return Ok(json!({
                                "job_id": job_id,
                                "status": "done",
                                "completed": completed,
                                "failed": failed,
                                "total_cost_usd": manifest.total_cost_usd,
                            }));
                        }
                        None => break,
                        _ => {}
                    }
                }
            }

            Ok(json!({ "job_id": job_id, "status": "running" }))
        }

        "be_status" => {
            let job_id = args["job_id"].as_str()
                .ok_or_else(|| anyhow!("Missing 'job_id'"))?;
            let manifest = store.load_manifest(job_id)?;
            Ok(serde_json::to_value(&manifest)?)
        }

        "be_results" => {
            let job_id = args["job_id"].as_str()
                .ok_or_else(|| anyhow!("Missing 'job_id'"))?;
            let limit = args.get("limit").and_then(|l| l.as_u64()).map(|l| l as usize);
            let offset = args.get("offset").and_then(|o| o.as_u64()).unwrap_or(0) as usize;
            let failed_only = args.get("failed_only").and_then(|f| f.as_bool()).unwrap_or(false);

            let mut results = store.load_results(job_id)?;
            if failed_only {
                results.retain(|r| r.status == be_core::store::ResultStatus::Failed);
            }
            let results: Vec<_> = results
                .into_iter()
                .skip(offset)
                .take(limit.unwrap_or(usize::MAX))
                .collect();

            Ok(serde_json::to_value(results)?)
        }

        "be_test" => {
            let bee_name = args["bee"].as_str()
                .ok_or_else(|| anyhow!("Missing 'bee' argument"))?;
            let input_obj = args.get("input")
                .and_then(|i| i.as_object())
                .ok_or_else(|| anyhow!("Missing 'input' argument"))?;

            let bee = Bee::find(bee_name)?;
            let mut input = std::collections::HashMap::new();
            for (k, v) in input_obj {
                input.insert(k.clone(), match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                });
            }

            let result = be_core::runner::run_bee(&bee, input, 0).await?;
            Ok(serde_json::to_value(result)?)
        }

        other => Err(anyhow!("Unknown tool: '{}'", other)),
    }
}
