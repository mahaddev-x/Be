# BeeHive

> **Public Beta — v0.5.0**
> Core features are stable. We're looking for early collaborators and testers before 1.0.
> [Report bugs](https://github.com/mahaddev-x/beehive/issues) · [Join the discussion](https://github.com/mahaddev-x/beehive/discussions) · [Contribute](docs/contributing.md)

**Run hundreds of independent AI tasks in true parallel.**

BeeHive is a local-first CLI for batch AI workloads. You define agents called **Bees** — each one is a stateless, single-purpose pi-agent that takes structured input, calls an LLM with full tool support, and returns validated JSON. Run one Bee or ten thousand simultaneously.

```bash
beehive run sentiment-scorer --input reviews.csv --parallel 200
```

---

## How it works

BeeHive is built on top of **[pi-mono](https://github.com/badlogic/lemmy)** by Mario Zechner — a production-grade TypeScript agent framework. Every Bee you define becomes a fully-featured **pi-agent**:

- **`@mariozechner/pi-ai`** routes LLM calls across 20+ providers (Groq, Anthropic, OpenAI, OpenRouter, Google, Mistral, xAI, Cerebras, Ollama, and more) through a unified API. You switch models by changing one string.
- **`@mariozechner/pi-agent-core`** gives each Bee a complete agent loop — stateful multi-turn conversations, structured tool calling, event-driven streaming, and automatic retry handling. BeeHive doesn't reimplement any of this — it exposes it directly.
- **`@mariozechner/pi-tui`** renders live streaming output in `beehive bee test` using a differential terminal renderer.

When you run `beehive run sentiment-scorer --input reviews.csv --parallel 50`, BeeHive creates 50 pi-agent instances simultaneously — each processing one row with its own conversation state, tool access, and retry logic. Results are saved to `~/.beehive/jobs/` the moment each agent finishes.

```
reviews.csv (1000 rows)
  │
  ├─ BeeHive dispatcher  (p-limit, concurrency=50)
  │    ├─ pi-agent #1  →  groq/llama-3.1-8b-instant  →  { sentiment: "positive", score: 0.95 }
  │    ├─ pi-agent #2  →  groq/llama-3.1-8b-instant  →  { sentiment: "negative", score: 0.82 }
  │    ├─ ...
  │    └─ pi-agent #50 →  groq/llama-3.1-8b-instant  →  { sentiment: "neutral",  score: 0.61 }
  │
  └─ ~/.beehive/jobs/<job-id>/results.jsonl
```

---

## Install

### Option A — npm (requires Node.js 20+)

```bash
npm install -g beehive-cli
beehive setup
```

### Option B — Download binary (no Node.js required)

Download the pre-built binary for your platform from the [Releases](https://github.com/mahaddev-x/beehive/releases) page:

| Platform | File |
|---|---|
| Windows 10/11 (x64) | `beehive-v*-bun-windows-x64.zip` |
| macOS Apple Silicon (M1/M2/M3/M4) | `beehive-v*-bun-darwin-arm64.tar.gz` |
| macOS Intel | `beehive-v*-bun-darwin-x64.tar.gz` |
| Linux x64 (Ubuntu, Debian, Arch, Kali, Fedora…) | `beehive-v*-bun-linux-x64.tar.gz` |
| Linux ARM64 (Raspberry Pi 4/5, AWS Graviton…) | `beehive-v*-bun-linux-arm64.tar.gz` |

**Windows:** Extract the zip, move `beehive.exe` to a folder on your `PATH`.

**macOS / Linux:**
```bash
tar -xzf beehive-v*-bun-darwin-arm64.tar.gz
sudo mv beehive/beehive /usr/local/bin/
beehive setup
```

---

## Quick start

**1. Set up your API keys**

```bash
beehive setup
```

Runs an interactive wizard, saves keys to `~/.beehive/config.toml`.
Fastest way to start: [Groq](https://console.groq.com) — free tier, ~200 tokens/sec.

**2. List available Bees**

```bash
beehive bee list
```

**3. Test a Bee interactively (streams live)**

```bash
beehive bee test sentiment-scorer --input '{"text":"This product is amazing!"}'
```

**4. Run a swarm**

```bash
beehive run sentiment-scorer --input reviews.csv --parallel 100
```

**5. View results**

```bash
beehive results <job-id>
```

---

## Bee definitions

A Bee is a YAML file that defines a pi-agent. Bees live in `./bees/` (project-local) or `~/.beehive/bees/` (global).

```bash
beehive bee new          # interactive wizard
beehive bee list         # list all bees
beehive bee show <name>  # inspect a bee
```

Example Bee:

```yaml
name: sentiment-scorer
description: Score the sentiment of a piece of text.

model: groq/llama-3.1-8b-instant
temperature: 0.1
max_tokens: 256

output_format: json
output_schema:
  type: object
  properties:
    sentiment:
      type: string
      enum: [positive, negative, neutral]
    score:
      type: number
    reason:
      type: string
  required: [sentiment, score, reason]

system_prompt: |
  You are a sentiment analysis engine. Respond with JSON only.

user_prompt_template: |
  Analyze the sentiment of this text:
  {{text}}

retry:
  max_attempts: 3
  backoff_seconds: 2
```

### Bee fields

| Field | Description |
|---|---|
| `model` | `provider/model-name` |
| `temperature` | 0.0–1.0 (use 0.0–0.2 for structured output) |
| `max_tokens` | Max response tokens |
| `system_prompt` | Agent instructions |
| `user_prompt_template` | `{{variable}}` replaced with input fields |
| `output_format` | `text` or `json` |
| `output_schema` | JSON Schema — validated on parsed output |
| `tools` | Built-in tools: `fetch_url`, `read_file` |
| `plugins` | Paths to custom tool `.ts`/`.js` files |
| `retry.max_attempts` | Retry count on failure |
| `retry.backoff_seconds` | Wait between retries |

### Built-in tools

| Tool | What it does |
|---|---|
| `fetch_url` | HTTP GET → Readability (strips nav/ads) → Markdown. Same pipeline as Claude Code. |
| `read_file` | Reads a local file, truncated at 20K chars |

---

## Built-in Bees

| Bee | Input |
|---|---|
| `sentiment-scorer` | `text` |
| `text-classifier` | `text`, `categories` |
| `data-extractor` | `text`, `fields` |
| `url-scraper` | `url` |
| `file-reviewer` | `path`, `focus` |

---

## Supported providers

`provider/model-name` format. Powered by pi-ai's routing layer.

| Provider | Example |
|---|---|
| Groq | `groq/llama-3.1-8b-instant` |
| Anthropic | `anthropic/claude-haiku-4-5-20251001` |
| OpenAI | `openai/gpt-4o-mini` |
| Google | `google/gemini-2.0-flash` |
| OpenRouter | `openrouter/meta-llama/llama-3.3-70b-instruct` |
| xAI | `xai/grok-3-mini` |
| Cerebras | `cerebras/llama-3.3-70b` |
| Mistral | `mistral/mistral-small-latest` |
| Ollama (local) | `ollama/qwen2.5:3b` |

---

## CLI reference

```
beehive setup                          Interactive API key wizard
beehive run <bee> [options]            Run a bee against a dataset
  --input <file>                       CSV, JSON array, or JSONL
  --input-json '<json>'                Inline JSON array
  --from-stdin                         Read from stdin
  --parallel <n>                       Concurrency (default: 50)
  --model <model>                      Override model
  --output <file>                      Write results to file
  --format table|json|csv|jsonl        Output format
  --quiet                              No progress output

beehive bee list                       List all bees
beehive bee show <name>                Print bee definition
beehive bee test <name>                Test one bee (live stream)
  --input '<json>'                     Input as JSON object
  --model <model>                      Override model
beehive bee new                        Create a bee interactively
beehive bee create <file>              Load a bee YAML

beehive results <job-id>               Show job results
  --format table|json|csv|jsonl
  --output <file>
  --failed-only

beehive jobs list                      List past jobs
beehive jobs show <id>                 Show job metadata
beehive jobs delete <id>               Delete a job

beehive config show                    Print config
beehive config set <key> <value>       Set a value
beehive config get <key>               Get a value
```

---

## Input formats

**CSV** — headers become variable names
```csv
text
Great product.
Terrible quality.
```

**JSON array**
```json
[{"text": "Great product."}, {"text": "Terrible quality."}]
```

**JSONL** — one object per line
```
{"url": "https://example.com"}
{"url": "https://news.ycombinator.com"}
```

---

## Configuration

`~/.beehive/config.toml` — edit directly or use `beehive config set`.

```toml
[providers]
groq_api_key      = "gsk_..."
anthropic_api_key = "sk-ant-..."
openrouter_api_key = "sk-or-..."
ollama_url        = "http://localhost:11434"

[defaults]
model           = "groq/llama-3.1-8b-instant"
parallel        = 50
timeout_seconds = 30
max_retries     = 3
```

---

## Contributing

We welcome contributors! Please read [CONTRIBUTING.md](docs/contributing.md) first.

- **Bug reports** → [GitHub Issues](https://github.com/mahaddev-x/beehive/issues)
- **Feature requests** → [GitHub Discussions](https://github.com/mahaddev-x/beehive/discussions)
- **Pull requests** → fork, branch, PR against `main`

```bash
git clone https://github.com/mahaddev-x/beehive.git
cd beehive
npm install
npm run dev          # run from source (tsx, no build needed)
npm run build        # compile to dist/
npm link             # put beehive on your PATH from source
```

---

## Platform support

| Platform | npm install | Binary |
|---|---|---|
| Windows 10/11 x64 | ✓ | ✓ `.exe` |
| macOS Apple Silicon | ✓ | ✓ |
| macOS Intel | ✓ | ✓ |
| Linux x64 | ✓ | ✓ |
| Linux ARM64 | ✓ | ✓ |

> Windows ARM64 and 32-bit platforms are not supported by Bun's compiler yet.
> The npm install path works on any platform with Node.js 20+.

---

## License

MIT
