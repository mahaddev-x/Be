# BeeHive — Bees, Tools & Extending Everything

This guide covers everything about creating Bees, using tools, writing custom tools,
and extending BeeHive with your own logic.

---

## Table of contents

1. [What is a Bee?](#1-what-is-a-bee)
2. [Creating a Bee from the CLI](#2-creating-a-bee-from-the-cli)
3. [Writing a Bee YAML by hand](#3-writing-a-bee-yaml-by-hand)
4. [All Bee fields explained](#4-all-bee-fields-explained)
5. [Built-in tools](#5-built-in-tools)
6. [Adding tools to a Bee](#6-adding-tools-to-a-bee)
7. [Writing a custom tool (plugin)](#7-writing-a-custom-tool-plugin)
8. [Output schemas and JSON validation](#8-output-schemas-and-json-validation)
9. [Input variables and templates](#9-input-variables-and-templates)
10. [Where Bees live](#10-where-bees-live)
11. [Testing and running Bees](#11-testing-and-running-bees)
12. [Full examples](#12-full-examples)

---

## 1. What is a Bee?

A **Bee** is a YAML file that defines a pi-agent. When you run it, BeeHive creates
a full [`pi-agent-core`](https://github.com/badlogic/lemmy) `Agent` instance with:

- A system prompt
- A model (any of 20+ providers)
- A set of tools it can call
- An output format (text or JSON)
- Retry and timeout budgets

You give BeeHive a Bee and a list of inputs. It runs one agent per input, all in
parallel, and collects structured results.

```
Input row  →  pi-agent (system_prompt + tools + model)  →  output (JSON or text)
```

---

## 2. Creating a Bee from the CLI

The fastest way to get started:

```bash
beehive bee new
```

This asks you a few questions (name, description, model, prompts, output format)
and writes the YAML file to `./bees/<name>.yaml`.

To load and validate an existing YAML file:

```bash
beehive bee create path/to/my-bee.yaml
```

To list all available Bees:

```bash
beehive bee list
```

To inspect a Bee's full definition:

```bash
beehive bee show sentiment-scorer
```

---

## 3. Writing a Bee YAML by hand

Create a file in `./bees/` (project-local) or `~/.beehive/bees/` (global).
The filename is the Bee's name — `sentiment-scorer.yaml` → `beehive run sentiment-scorer`.

**Minimal Bee (text output):**

```yaml
name: summarizer
description: Summarize a piece of text in one sentence.

model: groq/llama-3.1-8b-instant
temperature: 0.3
max_tokens: 128

system_prompt: |
  You are a summarization engine.
  Respond with a single sentence summary. No preamble.

user_prompt_template: |
  Summarize this:
  {{text}}
```

**Bee with JSON output:**

```yaml
name: sentiment-scorer
description: Score sentiment of text.

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
      description: Confidence from 0.0 to 1.0
    reason:
      type: string
  required: [sentiment, score, reason]

system_prompt: |
  You are a sentiment analysis engine.
  Respond with JSON only — no markdown fences, no explanation.

user_prompt_template: |
  Analyze the sentiment of this text:
  {{text}}
```

**Bee with tools:**

```yaml
name: url-scraper
description: Fetch a URL and extract structured information.

model: groq/llama-3.3-70b-versatile
temperature: 0.2
max_tokens: 1024

tools:
  - fetch_url

output_format: json
output_schema:
  type: object
  properties:
    title:
      type: string
    summary:
      type: string
    key_points:
      type: array
      items:
        type: string
  required: [title, summary, key_points]

system_prompt: |
  You are a web content analyst.
  Use the fetch_url tool to get the page, then return structured JSON.

user_prompt_template: |
  Fetch and analyze: {{url}}
```

---

## 4. All Bee fields explained

```yaml
# ── Identity ──────────────────────────────────────────────────────────────────
name: my-bee               # Required. Snake_case. Used in all CLI commands.
description: What it does  # Optional. Shown in `beehive bee list`.
version: "1.0"             # Optional.

# ── Model ─────────────────────────────────────────────────────────────────────
model: groq/llama-3.1-8b-instant   # provider/model-name. See providers below.
temperature: 0.1                    # 0.0–1.0. Use low values for JSON output.
max_tokens: 512                     # Max tokens in the LLM response.
max_turns: 5                        # Max agent turns (default: unlimited).
timeout_seconds: 30                 # Wall-clock timeout per Bee (default: 60).

# ── Prompts ───────────────────────────────────────────────────────────────────
system_prompt: |
  Instructions for the agent. Markdown is fine.

user_prompt_template: |
  {{variable}} is replaced with input fields.
  You can use any column name from your CSV or JSON input.

# ── Output ────────────────────────────────────────────────────────────────────
output_format: json         # "json" or "text" (default: text)
output_schema:              # Optional JSON Schema. Validated on the response.
  type: object
  properties:
    field_name:
      type: string          # string, number, boolean, array, object
  required: [field_name]

# ── Tools ─────────────────────────────────────────────────────────────────────
tools:                      # Built-in tools the agent can call.
  - fetch_url
  - read_file

plugins:                    # Paths to your own custom tool files.
  - ./tools/my-tool.ts
  - ./tools/another-tool.js

# ── Retry ─────────────────────────────────────────────────────────────────────
retry:
  max_attempts: 3           # Retry count on failure (default: 3).
  backoff_seconds: 2        # Wait between retries (default: 2).
                            # Respects Retry-After headers from 429 responses.
```

---

## 5. Built-in tools

BeeHive ships two built-in tools you can add to any Bee with one line.

### `fetch_url`

Fetches a URL and returns the main content as clean Markdown.

**Pipeline:** HTTP GET → Mozilla Readability (strips nav/ads/sidebars) →
Turndown (HTML → Markdown) → truncate at 12,000 chars.

This is the same pipeline used by Claude Code and Codex CLI.

```yaml
tools:
  - fetch_url
```

**What the agent can call:**
```
fetch_url(url: "https://example.com")
```
Returns clean Markdown of the page's main article content.

---

### `read_file`

Reads a file from the local filesystem and returns its contents as text.
Truncated at 20,000 characters.

```yaml
tools:
  - read_file
```

**What the agent can call:**
```
read_file(path: "./data/report.txt")
read_file(path: "/absolute/path/to/file.md")
```

---

## 6. Adding tools to a Bee

Simply list them under `tools:` in your YAML:

```yaml
tools:
  - fetch_url      # built-in
  - read_file      # built-in
  - ./tools/search.ts   # your custom plugin
```

The agent decides when and how to call them. You don't need to change your prompts —
just make the tools available and describe what they do in the tool's `description` field.

For Bees that need web access:

```yaml
tools:
  - fetch_url

system_prompt: |
  You are a research assistant.
  Use fetch_url to retrieve pages before answering.
```

For Bees that process local files:

```yaml
tools:
  - read_file

user_prompt_template: |
  Review this file: {{path}}
  Focus on: {{aspect}}
```

---

## 7. Writing a custom tool (plugin)

A tool is any TypeScript or JavaScript file that exports an `AgentTool` object.
The interface is from `@mariozechner/pi-agent-core`.

### Minimal custom tool

Create `tools/my-tool.ts`:

```typescript
import { Type } from "@mariozechner/pi-ai";
import type { AgentTool } from "@mariozechner/pi-agent-core";

export const myTool: AgentTool = {
  name: "my_tool",           // Unique snake_case name
  label: "My Tool",          // Human-readable label (shown in progress)
  description:
    "What this tool does, in plain English. " +
    "The LLM reads this to decide when to call the tool.",

  // Define the parameters using TypeBox (same as JSON Schema)
  parameters: Type.Object({
    query: Type.String({ description: "The search query" }),
    limit: Type.Optional(Type.Number({ description: "Max results, default 10" })),
  }),

  // The execute function receives the tool call ID and parsed parameters
  execute: async (_toolCallId, params) => {
    const { query, limit = 10 } = params as { query: string; limit?: number };

    // Do your work here — API calls, DB queries, file reads, anything
    const results = await myApiCall(query, limit);

    // Return content (what the LLM sees) and details (for logging/UI)
    return {
      content: [{ type: "text" as const, text: JSON.stringify(results) }],
      details: { query, resultCount: results.length },
    };
  },
};

// Export as default OR as named export "tools" (array)
export default myTool;
```

### Tool with streaming progress

For long-running tools, stream partial results so the UI updates live:

```typescript
import type { AgentTool, AgentToolUpdateCallback } from "@mariozechner/pi-agent-core";

export const scrapeTool: AgentTool = {
  name: "scrape_site",
  label: "Scrape Site",
  description: "Scrape multiple pages from a website.",
  parameters: Type.Object({
    urls: Type.Array(Type.String()),
  }),

  execute: async (_id, params, _signal, update?: AgentToolUpdateCallback) => {
    const { urls } = params as { urls: string[] };
    const results: string[] = [];

    for (const url of urls) {
      const text = await fetchPage(url);
      results.push(text);

      // Stream progress to the UI
      update?.({
        content: [{ type: "text" as const, text: results.join("\n---\n") }],
        details: { fetched: results.length, total: urls.length },
      });
    }

    return {
      content: [{ type: "text" as const, text: results.join("\n---\n") }],
      details: { fetched: results.length },
    };
  },
};
```

### Exporting multiple tools from one file

```typescript
// tools/web-tools.ts
export const fetchTool: AgentTool = { ... };
export const searchTool: AgentTool = { ... };

// Export as array for BeeHive to pick up all of them
export const tools = [fetchTool, searchTool];
```

### Using the plugin in a Bee

```yaml
name: my-bee
plugins:
  - ./tools/my-tool.ts       # relative to where you run beehive
  - ./tools/web-tools.ts     # file exporting multiple tools

tools:
  # Still list built-ins here if you need them
  - fetch_url
```

BeeHive auto-imports the plugin file at runtime (via tsx — no build step needed)
and registers the exported tools with the agent.

---

## 8. Output schemas and JSON validation

When `output_format: json` is set, BeeHive:

1. Appends your schema to the system prompt so the LLM knows the exact shape
2. Strips markdown fences from the response
3. Parses the JSON
4. Soft-validates against `output_schema.properties` — missing fields get a
   `_missing_<field>: true` warning in the output (doesn't fail the job)

**Schema format** — standard JSON Schema:

```yaml
output_schema:
  type: object
  properties:
    name:
      type: string
    score:
      type: number
    tags:
      type: array
      items:
        type: string
    metadata:
      type: object
  required: [name, score]
```

**Tip:** Use `temperature: 0.0` or `temperature: 0.1` with JSON output.
Higher temperatures increase malformed JSON frequency.

---

## 9. Input variables and templates

Your `user_prompt_template` uses `{{variable}}` syntax.
Any column from your CSV, key from your JSON, or field from your JSONL
becomes a variable automatically.

**CSV input:**
```csv
text,author
"Great product!",Alice
"Terrible quality.",Bob
```

**Template:**
```yaml
user_prompt_template: |
  Review by {{author}}:
  "{{text}}"
  
  Analyze the sentiment.
```

**Inline input:**
```bash
beehive run my-bee --input-json '[{"text":"hello","author":"Alice"}]'
```

**Stdin (pipe):**
```bash
cat data.jsonl | beehive run my-bee --from-stdin
```

If a variable is missing from an input row, it stays as `{{variable}}` in the
prompt — the LLM will see it literally, so make sure your inputs match your template.

---

## 10. Where Bees live

BeeHive searches two locations, in this order:

| Location | Scope | When to use |
|---|---|---|
| `./bees/` | Project-local | Bees specific to one project |
| `~/.beehive/bees/` | Global | Bees you use across many projects |

BeeHive finds the first match. Project-local Bees take priority over global ones
with the same name.

You can also pass a full path:
```bash
beehive run /path/to/my-bee.yaml --input data.csv
beehive bee test /path/to/my-bee.yaml --input '{"text":"hello"}'
```

---

## 11. Testing and running Bees

**Test a single input (interactive, streams live):**
```bash
beehive bee test sentiment-scorer --input '{"text":"This is great!"}'

# Override model for this test only
beehive bee test sentiment-scorer \
  --input '{"text":"This is great!"}' \
  --model openrouter/meta-llama/llama-3.3-70b-instruct
```

**Run against a file:**
```bash
# CSV
beehive run sentiment-scorer --input reviews.csv

# JSON array
beehive run sentiment-scorer --input reviews.json

# JSONL
beehive run sentiment-scorer --input reviews.jsonl

# Inline
beehive run sentiment-scorer \
  --input-json '[{"text":"hello"},{"text":"world"}]'
```

**Control parallelism:**
```bash
beehive run sentiment-scorer --input big-dataset.csv --parallel 100
```

**Save results:**
```bash
beehive run sentiment-scorer --input data.csv \
  --output results.json \
  --format json

# Other formats: table (default), csv, jsonl
```

**View a completed job:**
```bash
beehive results <job-id>
beehive results <job-id> --failed-only
beehive results <job-id> --format jsonl --output failed.jsonl
```

---

## 12. Full examples

### Research assistant (fetches URLs + extracts data)

```yaml
name: research-assistant
description: Fetch a URL and extract key facts as structured data.
model: groq/llama-3.3-70b-versatile
temperature: 0.1
max_tokens: 1024
tools:
  - fetch_url
output_format: json
output_schema:
  type: object
  properties:
    title: { type: string }
    date_published: { type: string }
    author: { type: string }
    summary: { type: string }
    key_claims:
      type: array
      items: { type: string }
  required: [title, summary, key_claims]
system_prompt: |
  You are a research extraction engine.
  Use fetch_url to retrieve the page content.
  Extract structured data and return JSON only.
user_prompt_template: |
  Extract information from this URL: {{url}}
```

```bash
beehive run research-assistant \
  --input-json '[{"url":"https://example.com/article"}]'
```

---

### Code reviewer (reads local files)

```yaml
name: code-reviewer
description: Review a source file and return structured feedback.
model: anthropic/claude-haiku-4-5-20251001
temperature: 0.2
max_tokens: 2048
tools:
  - read_file
output_format: json
output_schema:
  type: object
  properties:
    summary: { type: string }
    issues:
      type: array
      items: { type: string }
    suggestions:
      type: array
      items: { type: string }
    score:
      type: number
  required: [summary, issues, suggestions, score]
system_prompt: |
  You are a senior code reviewer.
  Use read_file to read the file, then provide structured feedback.
  Be specific — include line numbers and concrete suggestions.
user_prompt_template: |
  Review this file: {{path}}
  Focus on: {{focus}}
```

```bash
# Build a list of files to review
find . -name "*.ts" | jq -R '{path: ., focus: "correctness and types"}' | \
  jq -s . > files.json

beehive run code-reviewer --input files.json --parallel 10
```

---

### Custom tool: database lookup

```typescript
// tools/db-lookup.ts
import { Type } from "@mariozechner/pi-ai";
import type { AgentTool } from "@mariozechner/pi-agent-core";
import Database from "better-sqlite3";

const db = new Database("./data/products.db");

export default {
  name: "lookup_product",
  label: "Lookup Product",
  description:
    "Look up a product by ID or name in the database. " +
    "Returns price, stock, and description.",
  parameters: Type.Object({
    query: Type.String({ description: "Product name or ID to look up" }),
  }),
  execute: async (_id, params) => {
    const { query } = params as { query: string };
    const row = db
      .prepare("SELECT * FROM products WHERE name LIKE ? OR id = ? LIMIT 1")
      .get(`%${query}%`, query);

    const text = row
      ? JSON.stringify(row, null, 2)
      : `No product found for "${query}"`;

    return {
      content: [{ type: "text" as const, text }],
      details: { query, found: !!row },
    };
  },
} satisfies AgentTool;
```

```yaml
name: product-analyst
description: Analyze customer questions about products.
model: groq/llama-3.3-70b-versatile
plugins:
  - ./tools/db-lookup.ts
system_prompt: |
  You are a product support agent.
  Use lookup_product to find product information before answering.
user_prompt_template: |
  Customer question: {{question}}
```

```bash
beehive run product-analyst \
  --input-json '[{"question":"How much does the Pro plan cost?"}]'
```
