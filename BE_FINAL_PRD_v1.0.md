# Be — Final PRD & Architecture v1.0
## The complete, single source of truth. Hand this to Claude Code.

---

## 1. What Is Be

Be is a local-first, open-source, MIT-licensed platform for running
hundreds of independent, minimal AI tasks in true parallel.

Each task unit is called a **Bee**. A Bee is:
- Stateless — no memory, no history, no state between runs
- Single-purpose — one job, one output
- Independent — bees do not talk to each other, ever
- Fast — runs on small, cheap, fast models (Groq/Llama, Ollama local)
- Structured — always returns validated JSON matching a schema

You give Be a bee definition + a list of inputs + a parallelism count.
Be spawns N bees simultaneously. Each bee gets one input, processes it,
returns one structured JSON output. You get all outputs when they finish.

**The mental model:** bees in a hive. Each bee does exactly one job.
None of them know about the others. Together they get enormous work
done in parallel.

**This is NOT:**
- A coding agent (not like Claude Code, Cursor, Aider)
- A multi-agent system (bees never coordinate or communicate)
- A conversational AI
- A cloud service

---

## 2. Core Architecture Decision

### No client-server. No daemon. No server process. Ever.

The entire project is a **Rust workspace** with two crates:

```
be-core    →   library crate   (all engine logic)
be-cli     →   binary crate    (CLI, imports be-core directly)
```

`be-cli` calls `be-core` as a Rust library — same process, direct
function calls, zero network overhead, zero port conflicts, zero
process management. This is how Cargo itself is built. This is how
ripgrep, fd, bat are built. One binary. `cargo install be`. Done.

**Future GUI** (not in this version): will also be a binary crate
that imports `be-core` as a library. Same pattern.

---

## 3. Repository Structure

```
be/
├── Cargo.toml                    ← workspace manifest
├── Cargo.lock
│
├── crates/
│   ├── be-core/                  ← library crate: the engine
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── bee.rs            ← Bee struct + YAML loader/validator
│   │       ├── dispatcher.rs     ← Tokio parallel task spawner
│   │       ├── runner.rs         ← single bee execution
│   │       ├── store.rs          ← flat-file job/result storage
│   │       ├── config.rs         ← ~/.be/config.toml
│   │       ├── schema.rs         ← output JSON validation
│   │       ├── llm/
│   │       │   ├── mod.rs        ← unified LLM trait + routing
│   │       │   ├── openai.rs     ← OpenAI compat (Groq, Ollama, Together, OpenRouter, DeepSeek, OpenAI)
│   │       │   ├── anthropic.rs  ← Anthropic Messages API
│   │       │   └── google.rs     ← Google Generative AI API
│   │       └── tools/
│   │           ├── mod.rs
│   │           ├── fetch_url.rs  ← HTTP GET, returns cleaned text
│   │           └── read_file.rs  ← read local file, return content
│   │
│   └── be-cli/                   ← binary crate: the CLI
│       ├── Cargo.toml            ← depends on be-core
│       └── src/
│           ├── main.rs
│           ├── commands/
│           │   ├── run.rs        ← be run
│           │   ├── bee.rs        ← be bee new/list/show/test
│           │   ├── results.rs    ← be results
│           │   ├── jobs.rs       ← be jobs
│           │   ├── config.rs     ← be config
│           │   └── mcp.rs        ← be mcp (stdio MCP server)
│           └── tui/
│               ├── mod.rs
│               └── grid.rs       ← live bee grid (ratatui)
│
└── bees/                         ← built-in bee definitions
    ├── url-scraper.yaml
    ├── file-reviewer.yaml
    ├── text-classifier.yaml
    ├── data-extractor.yaml
    └── sentiment-scorer.yaml
```

**Cargo.toml (workspace root):**
```toml
[workspace]
members = ["crates/be-core", "crates/be-cli"]
resolver = "2"
```

---

## 4. be-core — The Engine (Library Crate)

### 4.1 bee.rs — Bee Definition

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Bee {
    pub name: String,
    pub version: String,
    pub description: String,
    pub model: String,                    // "groq/llama-3.1-8b-instant"
    pub system_prompt: String,
    pub user_prompt_template: String,     // uses {{variable_name}} syntax
    pub input_vars: Vec<InputVar>,
    pub output_schema: serde_json::Value, // JSON Schema object
    pub tools: Vec<String>,               // max 2: "fetch_url", "read_file"
    pub temperature: f32,                 // default 0.1
    pub max_tokens: u32,                  // default 512
    pub timeout_seconds: u64,             // default 30
    pub max_parallel: usize,              // hard cap for this bee
    pub retry: RetryConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InputVar {
    pub name: String,
    pub type_: String,    // "string", "integer", "boolean"
    pub required: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RetryConfig {
    pub max_attempts: u32,       // default 3
    pub backoff_seconds: u64,    // default 2
}

impl Bee {
    pub fn from_yaml_file(path: &Path) -> Result<Self>;
    pub fn from_yaml_str(s: &str) -> Result<Self>;
    pub fn validate(&self) -> Result<()>;
    pub fn interpolate_prompt(&self, input: &BeeInput) -> String;
}
```

Bee lookup order (first match wins):
1. `./bees/` — project-local bees
2. `~/.be/bees/` — user's global custom bees
3. Built-in bees (compiled into the binary as embedded YAML)

### 4.2 dispatcher.rs — Parallel Task Spawner

```rust
pub struct DispatchConfig {
    pub parallel: usize,                      // concurrent bees (1–1000)
    pub progress_tx: mpsc::Sender<BeeEvent>,  // live status events
}

#[derive(Debug, Clone)]
pub enum BeeEvent {
    Started    { job_id: String, index: usize },
    Completed  { job_id: String, index: usize, duration_ms: u64 },
    Failed     { job_id: String, index: usize, error: String },
    JobDone    { job_id: String, completed: usize, failed: usize },
}

pub async fn dispatch(
    bee: Bee,
    inputs: Vec<BeeInput>,
    config: DispatchConfig,
    store: Arc<JobStore>,
) -> Result<String> {  // returns job_id
    let job_id = store.create_job(&bee, inputs.len(), config.parallel).await?;
    let semaphore = Arc::new(Semaphore::new(config.parallel));

    for (index, input) in inputs.into_iter().enumerate() {
        let permit   = Arc::clone(&semaphore).acquire_owned().await?;
        let bee      = bee.clone();
        let store    = Arc::clone(&store);
        let tx       = config.progress_tx.clone();
        let job_id_c = job_id.clone();

        tokio::spawn(async move {
            let _permit = permit;
            tx.send(BeeEvent::Started {
                job_id: job_id_c.clone(), index
            }).ok();

            let result = runner::run_bee(&bee, input, index).await;

            match &result {
                Ok(r)  => tx.send(BeeEvent::Completed {
                    job_id: job_id_c.clone(),
                    index,
                    duration_ms: r.duration_ms,
                }).ok(),
                Err(e) => tx.send(BeeEvent::Failed {
                    job_id: job_id_c.clone(),
                    index,
                    error: e.to_string(),
                }).ok(),
            };

            store.write_result(&job_id_c, result).await.ok();
        });
    }

    Ok(job_id)
}
```

### 4.3 runner.rs — Single Bee Execution

```rust
pub async fn run_bee(
    bee: &Bee,
    input: BeeInput,
    index: usize,
) -> Result<BeeResult> {
    let start = Instant::now();
    let mut attempt = 0;

    loop {
        attempt += 1;
        match try_run_bee(bee, &input, index).await {
            Ok(result) => return Ok(result),
            Err(e) if attempt < bee.retry.max_attempts => {
                tokio::time::sleep(
                    Duration::from_secs(bee.retry.backoff_seconds)
                ).await;
                continue;
            }
            Err(e) => return Err(e),
        }
    }
}

async fn try_run_bee(
    bee: &Bee,
    input: &BeeInput,
    index: usize,
) -> Result<BeeResult> {
    let start = Instant::now();

    // Step 1: Run pre-tools to enrich input (fetch_url, read_file)
    let enriched = tools::run_pre_tools(bee, input).await?;

    // Step 2: Interpolate prompt template with input variables
    let user_prompt = bee.interpolate_prompt(&enriched);

    // Step 3: Call LLM
    let llm_response = llm::complete(LlmRequest {
        model:       &bee.model,
        system:      &bee.system_prompt,
        user:        &user_prompt,
        temperature: bee.temperature,
        max_tokens:  bee.max_tokens,
        timeout:     Duration::from_secs(bee.timeout_seconds),
    }).await?;

    // Step 4: Validate output JSON against bee's output_schema
    let validated_output = schema::validate_and_parse(
        &llm_response.content,
        &bee.output_schema,
    )?;

    Ok(BeeResult {
        index,
        status:       ResultStatus::Success,
        input:        input.clone(),
        output:       Some(validated_output),
        error:        None,
        tokens_input: llm_response.tokens_input,
        tokens_output: llm_response.tokens_output,
        cost_usd:     llm_response.cost_usd,
        duration_ms:  start.elapsed().as_millis() as u64,
        model_used:   bee.model.clone(),
        completed_at: Utc::now(),
    })
}
```

### 4.4 llm/ — Unified LLM Layer (pi-ai design, Rust implementation)

**Four real API formats cover every provider:**
1. OpenAI Completions — Groq, Ollama, Together, OpenRouter, DeepSeek, OpenAI
2. OpenAI Responses API — newer OpenAI format
3. Anthropic Messages API — all Claude models
4. Google Generative AI API — all Gemini models

```rust
// Unified trait
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, req: &LlmRequest) -> Result<LlmResponse>;
    fn cost_per_million_tokens(&self, model: &str) -> (f64, f64); // (input, output)
}

// LlmRequest (passed to every provider)
pub struct LlmRequest<'a> {
    pub model:       &'a str,   // model name without provider prefix
    pub system:      &'a str,
    pub user:        &'a str,
    pub temperature: f32,
    pub max_tokens:  u32,
    pub timeout:     Duration,
}

// LlmResponse (returned from every provider)
pub struct LlmResponse {
    pub content:       String,
    pub tokens_input:  u32,
    pub tokens_output: u32,
    pub cost_usd:      f64,
}

// Route model string to correct provider
pub async fn complete(req: LlmRequest<'_>) -> Result<LlmResponse> {
    let (provider_name, model_name) = req.model
        .split_once('/')
        .ok_or_else(|| anyhow!(
            "Invalid model format. Use: provider/model-name\n\
             Examples: groq/llama-3.1-8b-instant, ollama/qwen2.5:3b"
        ))?;

    let config = config::load()?;
    let provider: Box<dyn LlmProvider> = match provider_name {
        "groq"       => Box::new(OpenAiProvider::new(
            "https://api.groq.com/openai/v1",
            config.groq_api_key()?)),
        "ollama"     => Box::new(OpenAiProvider::new(
            &config.ollama_url(),
            "")),  // Ollama needs no auth
        "openrouter" => Box::new(OpenAiProvider::new(
            "https://openrouter.ai/api/v1",
            config.openrouter_api_key()?)),
        "together"   => Box::new(OpenAiProvider::new(
            "https://api.together.xyz/v1",
            config.together_api_key()?)),
        "deepseek"   => Box::new(OpenAiProvider::new(
            "https://api.deepseek.com/v1",
            config.deepseek_api_key()?)),
        "openai"     => Box::new(OpenAiProvider::new(
            "https://api.openai.com/v1",
            config.openai_api_key()?)),
        "anthropic"  => Box::new(AnthropicProvider::new(
            config.anthropic_api_key()?)),
        "google"     => Box::new(GoogleProvider::new(
            config.google_api_key()?)),
        other        => return Err(anyhow!(
            "Unknown provider: '{}'. Supported: groq, ollama, openrouter, \
             together, deepseek, openai, anthropic, google", other)),
    };

    provider.complete(&LlmRequest { model: model_name, ..req }).await
}
```

**Supported models for bees (fast + cheap focus):**

| Model string | Provider | Speed | Cost | Best for |
|---|---|---|---|---|
| `groq/llama-3.1-8b-instant` | Groq | ⚡ Fastest API | ~$0.05/M | Default for most bees |
| `groq/llama-3.3-70b-versatile` | Groq | Fast | ~$0.59/M | Bees needing reasoning |
| `groq/gemma2-9b-it` | Groq | Fast | ~$0.20/M | Classification bees |
| `ollama/qwen2.5:3b` | Local | Fast (GPU) | Free | Private data, local GPU |
| `ollama/llama3.2:3b` | Local | Fast (GPU) | Free | Private data, local GPU |
| `deepseek/deepseek-v3` | DeepSeek | Medium | ~$0.01/M | Cheapest paid option |
| `anthropic/claude-haiku-4-5` | Anthropic | Medium | ~$0.25/M | Quality-critical bees |
| `google/gemini-flash-2.0` | Google | Fast | ~$0.10/M | Multimodal (images+text) |
| `openrouter/...` | OpenRouter | Variable | Model-dep | Access to 200+ models |

### 4.5 tools/ — Pre-execution Tool Runtime

Tools run BEFORE the LLM call to enrich the bee's input. Not LLM tool
calls — just Rust functions that fetch/read data.

```rust
pub async fn run_pre_tools(bee: &Bee, input: &BeeInput) -> Result<BeeInput> {
    let mut enriched = input.clone();
    for tool_name in &bee.tools {
        match tool_name.as_str() {
            "fetch_url" => {
                // Requires {{url}} in input_vars
                // Fetches URL, cleans HTML to text, puts in {{content}}
                if let Some(url) = enriched.get("url") {
                    let content = fetch_url::fetch(url).await?;
                    enriched.insert("content", content);
                }
            }
            "read_file" => {
                // Requires {{file_path}} in input_vars
                // Reads file, puts content in {{file_content}}
                if let Some(path) = enriched.get("file_path") {
                    let content = read_file::read(path)?;
                    enriched.insert("file_content", content);
                }
            }
            other => return Err(anyhow!("Unknown tool: {}", other)),
        }
    }
    Ok(enriched)
}
```

### 4.6 store.rs — Flat-File Storage

**No SQLite. No database. Plain files.**

Why: SQLite only allows one concurrent writer. When 500 bees finish
simultaneously, every write queues behind a lock — destroying Be's
core value prop. Flat files allow true parallel writes: one file per
bee, each bee writes independently, zero contention.

```
~/.be/
├── config.toml
└── jobs/
    └── <job-uuid>/
        ├── manifest.json    ← job metadata
        └── results/
            ├── 000000.json  ← bee #0 result
            ├── 000001.json  ← bee #1 result
            └── ...          ← each written independently
```

**manifest.json:**
```json
{
  "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "bee_name": "url-scraper",
  "model": "groq/llama-3.1-8b-instant",
  "total": 500,
  "parallel": 100,
  "status": "running",
  "created_at": "2026-04-08T14:22:01Z",
  "finished_at": null,
  "completed": 0,
  "failed": 0,
  "total_cost_usd": 0.0,
  "avg_duration_ms": 0
}
```

Status values: `"running"` → `"done"` or `"done_with_errors"` or `"failed"`

**results/000042.json:**
```json
{
  "index": 42,
  "status": "success",
  "input": { "url": "https://example.com/page" },
  "output": {
    "title": "Example Page",
    "description": "...",
    "main_topic": "technology",
    "key_points": ["point1", "point2"],
    "word_count": 847,
    "language": "en"
  },
  "tokens_input": 312,
  "tokens_output": 89,
  "cost_usd": 0.000062,
  "duration_ms": 1180,
  "model_used": "groq/llama-3.1-8b-instant",
  "completed_at": "2026-04-08T14:22:03.241Z"
}
```

Failed bee:
```json
{
  "index": 7,
  "status": "failed",
  "input": { "url": "https://broken.com" },
  "output": null,
  "error": "fetch_url: timeout after 30s",
  "retries_attempted": 3,
  "duration_ms": 90041,
  "completed_at": "2026-04-08T14:23:01.012Z"
}
```

```rust
pub struct JobStore {
    base_dir: PathBuf,  // ~/.be/jobs/
}

impl JobStore {
    pub fn new() -> Result<Self>;

    // Create job dir + manifest
    pub async fn create_job(
        &self,
        bee: &Bee,
        total: usize,
        parallel: usize,
    ) -> Result<String>;  // returns job_id

    // Write one bee result (called concurrently from many tokio tasks)
    pub async fn write_result(
        &self,
        job_id: &str,
        result: Result<BeeResult>,
    ) -> Result<()>;

    // Mark job complete, update manifest
    pub async fn finish_job(
        &self,
        job_id: &str,
        stats: JobStats,
    ) -> Result<()>;

    // Load all results for a job (sorted by index)
    pub fn load_results(&self, job_id: &str) -> Result<Vec<BeeResult>>;

    // Load job manifest
    pub fn load_manifest(&self, job_id: &str) -> Result<JobManifest>;

    // List all jobs, newest first
    pub fn list_jobs(&self) -> Result<Vec<JobManifest>>;

    // Delete a job and all its results
    pub fn delete_job(&self, job_id: &str) -> Result<()>;
}
```

### 4.7 config.rs

Config file: `~/.be/config.toml`

```toml
[providers]
groq_api_key       = ""
anthropic_api_key  = ""
openai_api_key     = ""
openrouter_api_key = ""
together_api_key   = ""
deepseek_api_key   = ""
google_api_key     = ""
ollama_url         = "http://localhost:11434"

[defaults]
model              = "groq/llama-3.1-8b-instant"
parallel           = 50
timeout_seconds    = 30
max_retries        = 3
```

### 4.8 schema.rs — Output Validation

Every bee result is validated against `output_schema` before being
saved. If the LLM returns invalid JSON or missing required fields,
it triggers a retry.

```rust
pub fn validate_and_parse(
    llm_output: &str,
    schema: &serde_json::Value,
) -> Result<serde_json::Value> {
    // Strip markdown code fences if present (```json ... ```)
    let cleaned = strip_code_fences(llm_output);

    // Parse as JSON
    let value: serde_json::Value = serde_json::from_str(&cleaned)
        .map_err(|e| anyhow!("LLM output is not valid JSON: {}", e))?;

    // Validate required fields from schema
    validate_against_schema(&value, schema)?;

    Ok(value)
}
```

### 4.9 Public API of be-core

```rust
// be-core/src/lib.rs

pub mod bee;
pub mod dispatcher;
pub mod runner;
pub mod llm;
pub mod tools;
pub mod store;
pub mod config;
pub mod schema;

// What be-cli uses:
pub use bee::Bee;
pub use dispatcher::{dispatch, DispatchConfig, BeeEvent};
pub use store::{JobStore, JobManifest, BeeResult};
pub use config::Config;
```

---

## 5. be-cli — The CLI (Binary Crate)

Imports `be-core` as a library. All heavy lifting is in be-core.
be-cli's job: parse user input, call be-core, display results.

### 5.1 All Commands

```
USAGE: be <COMMAND>

COMMANDS:
  setup              Interactive first-run setup wizard
  
  bee new            Interactive wizard to create a bee YAML file
  bee create <file>  Load/validate a bee from a YAML file
  bee list           List all available bees (built-in + custom)
  bee show <name>    Print bee definition
  bee edit <name>    Open bee YAML in $EDITOR
  bee test <name>    Run one bee on one input, show full result
  
  run <bee>          Run a swarm of bees
  
  results <job-id>   Show results of a completed job
  jobs               List all jobs
  jobs clear         Delete all job history
  
  config set <k> <v> Set a config value
  config get <k>     Get a config value
  config show        Show full config (API keys masked)
  config test        Test all provider connections
  
  mcp                Start MCP server on stdio
  
  version
  help [command]
```

### 5.2 be run — Full Options

```
be run <bee-name> [OPTIONS]

OPTIONS:
  --input <file>         Input file: CSV, JSON array, or JSONL
  --input-json '<[...]>' Inline JSON array input
  --from-stdin           Read JSON array from stdin
  --parallel <n>         Concurrent bees (default: 50, max: 1000)
  --model <model>        Override bee's model for this run
  --output <file>        Write results to file
  --format <fmt>         Output format: table (default) | json | csv | jsonl
  --watch                Show live TUI grid during execution
  --no-color             Plain text output

EXAMPLES:
  be run url-scraper --input urls.csv --parallel 200
  be run file-reviewer --input files.json --parallel 50 --watch
  be run text-classifier --input-json '[{"text":"hello"},{"text":"bye"}]'
  cat urls.txt | jq -R '{url:.}' | jq -s . | be run url-scraper --from-stdin
```

### 5.3 be setup — First-Run Wizard

```
Welcome to Be!

Which providers do you want to use?
  [x] Groq (fast, cheap, Llama/Mixtral/Gemma models)
  [x] Ollama (local, free, GPU-accelerated)
  [ ] Anthropic (Claude models, quality-focused)
  [ ] OpenAI
  [ ] OpenRouter (200+ models via one key)
  [ ] DeepSeek (very cheap)
  [ ] Together AI

Enter your Groq API key (free at console.groq.com):
> gsk_...

Ollama: checking http://localhost:11434 ...
  ✓ Found! Available models: qwen2.5:3b, llama3.2:3b

Config saved to ~/.be/config.toml ✓

Run your first bee:
  be bee test url-scraper --input '{"url":"https://example.com"}'
```

### 5.4 be bee new — Interactive Bee Builder

```
Creating a new bee.

Bee name (snake_case): product-analyzer

What does this bee do? (one sentence):
> Extracts product name, price, and availability from a product page URL

Which model?
  ● groq/llama-3.1-8b-instant  (fastest, cheapest — recommended)
  ○ groq/llama-3.3-70b-versatile
  ○ ollama/qwen2.5:3b  (local, free)
  ○ anthropic/claude-haiku-4-5
  ○ custom model string

Input variables:
  Add variable name (Enter to skip): url
  Type [string/integer/boolean]: string
  Required? [Y/n]: y
  Add variable name (Enter to skip): [Enter]

Output fields:
  Add field name (Enter to skip): product_name
  Type [string/integer/boolean/array]: string
  Add field name (Enter to skip): price
  Type: string
  Add field name (Enter to skip): in_stock
  Type: boolean
  Add field name (Enter to skip): [Enter]

Does this bee need to fetch the URL content first?
  ● Yes, add fetch_url tool
  ○ No

Writing to ./bees/product-analyzer.yaml ✓

Test it now? [Y/n]: y
Test input (JSON): {"url": "https://example.com/product"}

Running...

Result:
  product_name: "Example Widget Pro"
  price: "$29.99"
  in_stock: true

  Tokens: 401 in / 87 out  |  Cost: $0.0001  |  Time: 0.9s  |  Status: ✓

Run a swarm:
  be run product-analyzer --input products.csv --parallel 100
```

### 5.5 be run --watch — Live TUI Grid

```
Job a1b2c3d4 · url-scraper · groq/llama-3.1-8b-instant
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  ● ● ● ○ ● ✓ ✓ ✓ ✗ ● ● ○ ○ ● ✓ ✓ ● ● ○ ● ✓ ✓ ✓ ✓ ✓
  ✓ ✓ ✓ ● ● ✓ ● ○ ✓ ✓ ● ✓ ✓ ● ● ✓ ● ● ✓ ✓ ○ ● ● ✓ ●
  ...

  ● running    ✓ done    ✗ failed    ○ queued

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  247 / 500   ▓▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░  49.4%
  3 failed · avg 1.2s/bee · est. 3m 12s remaining
  Cost so far: $0.031

  [Q] quit  [P] pause  [R] retry failed
```

Implemented using `ratatui` + `crossterm`. Reads `BeeEvent` from the
mpsc channel that dispatcher sends to. Updates in real-time.

### 5.6 be results — Results Table

```
be results a1b2c3d4

#    STATUS  INPUT.URL                      OUTPUT.TITLE               COST     TIME
0    ✓       https://example.com/page-0     Example Widget Pro         $0.0001  1.2s
1    ✓       https://example.com/page-1     Another Product            $0.0001  0.9s
2    ✗       https://broken-site.com        —                          —        30.0s
...

500 total · 497 succeeded · 3 failed · total cost $0.031 · avg 1.2s/bee
```

Export:
```bash
be results <id> --format jsonl > results.jsonl
be results <id> --format csv > results.csv
be results <id> --format json > results.json

# Or use raw files directly (they're just JSON)
cat ~/.be/jobs/<id>/results/*.json | jq '.output.title'
```

### 5.7 be mcp — MCP Server

Starts an MCP server on stdio. Add to any AI agent's MCP config.

**MCP tools exposed:**

```json
{
  "tools": [
    {
      "name": "be_run",
      "description": "Deploy N bees to process a list of inputs in parallel. Returns job_id immediately.",
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
        "properties": { "job_id": { "type": "string" } },
        "required": ["job_id"]
      }
    },
    {
      "name": "be_results",
      "description": "Get results of a completed job.",
      "inputSchema": {
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
        "properties": {
          "bee":   { "type": "string" },
          "input": { "type": "object" }
        },
        "required": ["bee", "input"]
      }
    }
  ]
}
```

**Claude Code `.mcp.json` integration:**
```json
{
  "mcpServers": {
    "be": {
      "command": "be",
      "args": ["mcp"]
    }
  }
}
```

---

## 6. Bee Definition — Full YAML Schema

```yaml
# bees/url-scraper.yaml

name: url-scraper
version: "1.0"
description: "Fetches a URL and extracts structured content data"

# Model — format: "provider/model-name"
model: groq/llama-3.1-8b-instant

# Generation parameters
temperature: 0.1         # low = deterministic = reliable structured output
max_tokens: 512
timeout_seconds: 30

# Concurrency limits
max_parallel: 200        # CLI --parallel cannot exceed this

# Prompts — use {{variable_name}} for interpolation
system_prompt: |
  You are a precise content extractor. Given web page content,
  return ONLY a valid JSON object matching the output schema exactly.
  No explanation. No markdown. No text outside the JSON object.

user_prompt_template: |
  Extract structured data from this web page.

  URL: {{url}}
  Content: {{content}}

  Return JSON only.

# Input — what each row in your input file must provide
input_vars:
  - name: url
    type: string
    required: true
  - name: content
    type: string
    required: false    # fetch_url tool populates this automatically

# Output — Be validates every result against this
output_schema:
  title: string
  description: string
  main_topic: string
  key_points: array
  word_count: integer
  language: string

# Tools — run BEFORE the LLM call to enrich the input
# Maximum 2 tools per bee
tools:
  - fetch_url            # fetches {{url}} → puts result in {{content}}

# Retry policy
retry:
  max_attempts: 3
  backoff_seconds: 2
```

---

## 7. The 5 Built-in Bees

### Bee 1: url-scraper
- **Does:** Fetches a URL, extracts structured content
- **Model:** `groq/llama-3.1-8b-instant`
- **Tool:** `fetch_url`
- **Input:** `{ url: string }`
- **Output:** `{ title, description, main_topic, key_points[], word_count, language }`
- **Use case:** Scrape 500 product pages, monitor 200 sites, extract from 1000 articles

### Bee 2: file-reviewer
- **Does:** Reviews a source code file, returns issues and quality score
- **Model:** `groq/llama-3.3-70b-versatile`
- **Tool:** `read_file`
- **Input:** `{ file_path: string, language: string }`
- **Output:** `{ quality_score: 1-10, issues[], suggestions[], summary }`
- **Use case:** Audit every file in a codebase in parallel (1 bee per file)

### Bee 3: text-classifier
- **Does:** Classifies text into one of N provided categories
- **Model:** `groq/gemma2-9b-it`
- **Tool:** none
- **Input:** `{ text: string, categories: string[] }`
- **Output:** `{ category: string, confidence: 0.0-1.0, reasoning: string }`
- **Use case:** Classify 10,000 support tickets, label 50,000 rows, sort emails

### Bee 4: data-extractor
- **Does:** Extracts structured data from unstructured text
- **Model:** `groq/llama-3.1-8b-instant`
- **Tool:** none
- **Input:** `{ text: string, schema: object }`
- **Output:** whatever schema the user defines
- **Use case:** Extract prices from 1000 scraped pages, parse 500 invoices

### Bee 5: sentiment-scorer
- **Does:** Returns sentiment score, emotions, and tone summary
- **Model:** `ollama/qwen2.5:3b` (local, free, GPU-accelerated)
- **Tool:** none
- **Input:** `{ text: string }`
- **Output:** `{ sentiment: positive|negative|neutral, score: -1.0-1.0, emotions: string[], tone_summary: string }`
- **Use case:** Score 50,000 reviews, analyze 10,000 social posts

---

## 8. GPU Strategy

GPU matters only for **local model inference via Ollama**.

- For API calls (Groq, Anthropic, etc.): GPU is on their servers. Be
  just makes async HTTP calls. Your GPU is irrelevant.
- For local models via Ollama: Be sends 100 parallel requests to
  `localhost:11434`. Ollama batches them and runs on your GPU
  (NVIDIA CUDA, Apple Silicon Metal, AMD ROCm).

Setup for local GPU bees:
```bash
# Install Ollama from ollama.com
ollama pull qwen2.5:3b

be config set ollama.url http://localhost:11434

# Run 100 bees on local GPU — free and private
be run sentiment-scorer --input reviews.csv --parallel 100 \
    --model ollama/qwen2.5:3b
```

---

## 9. Full Rust Dependency List

```toml
# be-core/Cargo.toml
[dependencies]
tokio        = { version = "1", features = ["full"] }
reqwest      = { version = "0.12", features = ["json", "stream"] }
serde        = { version = "1", features = ["derive"] }
serde_json   = "1"
serde_yaml   = "0.9"
anyhow       = "1"
thiserror    = "1"
uuid         = { version = "1", features = ["v4"] }
chrono       = { version = "0.4", features = ["serde"] }
async-trait  = "0.1"
dirs         = "5"
tokio-util   = "0.7"

# be-cli/Cargo.toml
[dependencies]
be-core      = { path = "../be-core" }
clap         = { version = "4", features = ["derive"] }
ratatui      = "0.27"
crossterm    = "0.27"
indicatif    = "0.17"
dialoguer    = "0.11"
comfy-table  = "7"
colored      = "2"
tokio        = { version = "1", features = ["full"] }
anyhow       = "1"
serde_json   = "1"
csv          = "1"
```

No C dependencies. No bundled SQLite. No Node.js. No Python.
Pure Rust. Single binary.

---

## 10. Build, Install & Run

```bash
# Clone
git clone https://github.com/your-org/be
cd be

# Build
cargo build --release

# Install globally from source
cargo install --path crates/be-cli

# First-time setup
be setup

# Verify
be version
be bee list
```

---

## 11. Design Principles (Non-Negotiable)

1. **Bees are dumb by design.** No memory, no history, no loops,
   no coordination between bees. Simple = reliable = fast.

2. **Local-first forever.** No cloud dependency, no telemetry,
   no accounts required, no data leaving the machine.

3. **One binary.** `cargo install be` is the entire install.
   No Docker, no Node.js, no Python, no external dependencies.

4. **YAML is the API.** Defining a bee takes 2 minutes.
   No code required. Files are human-readable and git-friendly.

5. **Fast models first.** Groq/Llama/Ollama/Gemma are the primary
   targets. Bees are cheap workers, not senior engineers.
   Default model: `groq/llama-3.1-8b-instant`.

6. **Parallelism is the product.** Every architectural decision
   optimizes for "run 500 of these simultaneously."

7. **Transparent storage.** Results are plain JSON files.
   `cat`, `jq`, `grep` all work. No special tooling needed.

8. **Open source, MIT license.** No exceptions.

---

## 12. What Is NOT Being Built (MVP Scope)

- No background daemon / no server process
- No HTTP server / no REST API / no web dashboard
- No browser-based UI
- No desktop app (future crate, not now)
- No mobile
- No Docker requirement
- No cloud hosting / no managed service
- No authentication / no user accounts
- No agent-to-agent communication (bees never talk to each other)
- No embedded LLM inference engine (use Ollama for local GPU)
- No SQLite or any database
- No TypeScript / no Node.js dependency
- No conversation history / no session management (bees are stateless)
- No MCP client (Be is an MCP server only)

---

## 13. MVP Checklist for Claude Code

### be-core
- [ ] `bee.rs` — Bee struct, YAML loader, validator, prompt interpolator
- [ ] `dispatcher.rs` — Tokio parallel task spawner with semaphore
- [ ] `runner.rs` — Single bee execution with retry logic
- [ ] `llm/mod.rs` — LlmProvider trait + model string routing
- [ ] `llm/openai.rs` — OpenAI Completions compat (Groq, Ollama, Together, OpenRouter, DeepSeek, OpenAI)
- [ ] `llm/anthropic.rs` — Anthropic Messages API
- [ ] `llm/google.rs` — Google Generative AI API
- [ ] `tools/fetch_url.rs` — Async HTTP GET + HTML-to-text cleaning
- [ ] `tools/read_file.rs` — Local file reader
- [ ] `store.rs` — Flat-file job store (create, write_result, finish, load, list)
- [ ] `config.rs` — ~/.be/config.toml loader/writer
- [ ] `schema.rs` — JSON output validator

### be-cli
- [ ] `main.rs` — Clap root command setup
- [ ] `commands/run.rs` — `be run` with all options
- [ ] `commands/bee.rs` — `be bee new/list/show/test`
- [ ] `commands/results.rs` — `be results` with table/json/csv/jsonl output
- [ ] `commands/jobs.rs` — `be jobs` and `be jobs clear`
- [ ] `commands/config.rs` — `be config set/get/show/test`
- [ ] `commands/mcp.rs` — `be mcp` MCP server on stdio
- [ ] `tui/grid.rs` — Live bee grid (ratatui) for `be run --watch`
- [ ] `setup.rs` — `be setup` first-run wizard

### Built-in Bees (YAML files)
- [ ] `bees/url-scraper.yaml`
- [ ] `bees/file-reviewer.yaml`
- [ ] `bees/text-classifier.yaml`
- [ ] `bees/data-extractor.yaml`
- [ ] `bees/sentiment-scorer.yaml`

---

*Be v1.0 — Final PRD & Architecture*
*Project by Vortex (OUTLAW)*
*License: MIT*
*Status: Ready for implementation*
